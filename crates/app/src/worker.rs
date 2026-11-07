//! Drain the outbox in this process. A plugin's return value is posted back
//! as a signed callback; only that request is stored.

use std::sync::Arc;
use std::time::Duration;

use domain::{
    Ats, CallbackBody, EventName, EventRecord, ResumeFile, RunPayload, UploadedPayload, sign,
};
use plugins::structure;
use storage::{Objects, Store};

use crate::bus::Bus;

pub struct Worker {
    store: Store,
    objects: Arc<dyn Objects>,
    plugins: Vec<Arc<dyn Ats>>,
    bus: Bus,
    self_url: String,
    client: reqwest::Client,
}

impl Worker {
    pub fn new(
        store: Store,
        objects: Arc<dyn Objects>,
        plugins: Vec<Arc<dyn Ats>>,
        bus: Bus,
        self_url: impl Into<String>,
    ) -> Self {
        Self {
            store,
            objects,
            plugins,
            bus,
            self_url: self_url.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Keep claiming until the outbox is idle. Used by the spine test so an
    /// upload can reach a callback without a background loop.
    pub async fn drain(&self) -> anyhow::Result<()> {
        while self.step().await? {}
        Ok(())
    }

    pub fn spawn(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                match self.step().await {
                    Ok(true) => {}
                    Ok(false) => tokio::time::sleep(Duration::from_millis(200)).await,
                    Err(err) => {
                        eprintln!("worker: {err:#}");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        });
    }

    async fn step(&self) -> anyhow::Result<bool> {
        let Some(claimed) = self.store.claim().await? else {
            return Ok(false);
        };
        let event = EventRecord {
            id: claimed.id,
            resume_id: claimed.resume_id,
            version_id: claimed.version_id,
            name: claimed.name,
            payload: claimed.payload.clone(),
            created_at: claimed.created_at,
        };
        let result = self.handle(&event).await;
        match result {
            Ok(()) => self.store.finish(claimed.id, claimed.token).await?,
            Err(err) => {
                self.store.release(claimed.id, claimed.token).await?;
                return Err(err);
            }
        }
        self.bus.publish(event);
        Ok(true)
    }

    async fn handle(&self, event: &EventRecord) -> anyhow::Result<()> {
        match event.name {
            EventName::ResumeUploaded => self.uploaded(event).await,
            EventName::ResumeParsed => self.parsed(event).await,
            EventName::AtsRunRequested => self.requested(event).await,
            EventName::AtsCallbackReceived => Ok(()),
        }
    }

    async fn uploaded(&self, event: &EventRecord) -> anyhow::Result<()> {
        let payload: UploadedPayload = serde_json::from_value(event.payload.clone())?;
        let bytes = self.objects.get(&payload.object_key).await?;
        let parse = structure(&ResumeFile {
            media_type: &payload.media_type,
            bytes: &bytes,
        });
        self.store.save_parse(event.version_id, &parse).await
    }

    async fn parsed(&self, event: &EventRecord) -> anyhow::Result<()> {
        let ids: Vec<&str> = self.plugins.iter().map(|plugin| plugin.id()).collect();
        self.store.fanout(event.version_id, &ids).await
    }

    async fn requested(&self, event: &EventRecord) -> anyhow::Result<()> {
        let payload: RunPayload = serde_json::from_value(event.payload.clone())?;
        let plugin = self
            .plugins
            .iter()
            .find(|plugin| plugin.id() == payload.plugin_id)
            .ok_or_else(|| anyhow::anyhow!("unknown plugin {}", payload.plugin_id))?;
        let (object_key, media_type) = self.store.file_of(event.version_id).await?;
        let bytes = self.objects.get(&object_key).await?;
        let read = plugin.read(&ResumeFile {
            media_type: &media_type,
            bytes: &bytes,
        });
        let body = CallbackBody {
            run_id: payload.run_id,
            parse: read.parse,
            verdict: read.verdict,
        };
        let raw = serde_json::to_vec(&body)?;
        let timestamp = chrono::Utc::now().timestamp();
        let signature = sign(plugin.secret(), timestamp, &raw);
        let url = format!(
            "{}/api/callbacks/{}",
            self.self_url.trim_end_matches('/'),
            plugin.id()
        );
        let response = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .header("x-timestamp", timestamp.to_string())
            .header("x-signature", signature)
            .header("idempotency-key", payload.run_id.to_string())
            .body(raw)
            .send()
            .await?;
        anyhow::ensure!(
            response.status().is_success(),
            "callback for {} returned {}",
            payload.run_id,
            response.status()
        );
        Ok(())
    }
}

pub fn plugin_secret() -> String {
    std::env::var("PLUGIN_SECRET").unwrap_or_else(|_| "spine-stub-secret".to_owned())
}

pub fn compiled_plugins() -> Vec<Arc<dyn Ats>> {
    plugins::compiled(plugin_secret())
}
