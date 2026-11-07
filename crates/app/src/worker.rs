//! Drain the outbox in this process. A compiled plugin is called with the raw
//! file and the webhook URL. A private provider is handed the file and posts
//! the callback itself. Only that callback is stored.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use domain::{Ats, EventName, EventRecord, ResumeFile, RunPayload, UploadedPayload};
use plugins::structure;
use serde::Serialize;
use storage::{Objects, Store};
use uuid::Uuid;

use crate::bus::Bus;

pub struct Worker {
    store: Store,
    objects: Arc<dyn Objects>,
    plugins: Vec<Arc<dyn Ats>>,
    endpoints: HashMap<String, String>,
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
            endpoints: HashMap::new(),
            bus,
            self_url: self_url.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Private provider urls, keyed by plugin id. Empty keeps the in-process stub.
    pub fn endpoints(mut self, endpoints: HashMap<String, String>) -> Self {
        self.endpoints = endpoints;
        self
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
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown plugin {}", payload.plugin_id))?;
        let endpoint = self.endpoints.get(plugin.id()).cloned();
        let (object_key, media_type) = self.store.file_of(event.version_id).await?;
        let bytes = self.objects.get(&object_key).await?;
        if let Some(url) = endpoint {
            self.handoff(&url, plugin.id(), payload.run_id, &media_type, &bytes)
                .await?;
            return Ok(());
        }
        let webhook_url = format!(
            "{}/api/callbacks/{}",
            self.self_url.trim_end_matches('/'),
            plugin.id()
        );
        plugins::call(
            plugin.as_ref(),
            &ResumeFile {
                media_type: &media_type,
                bytes: &bytes,
            },
            &webhook_url,
            payload.run_id,
        )
        .await
    }

    /// The provider accepts, waits, then signs `POST /api/callbacks/{plugin_id}` itself.
    async fn handoff(
        &self,
        url: &str,
        plugin_id: &str,
        run_id: Uuid,
        media_type: &str,
        bytes: &[u8],
    ) -> anyhow::Result<()> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let callback_url = format!(
            "{}/api/callbacks/{plugin_id}",
            self.self_url.trim_end_matches('/')
        );
        let response = self
            .client
            .post(format!("{url}/"))
            .timeout(Duration::from_secs(5))
            .json(&ProviderJob {
                run_id,
                file: ProviderFile {
                    media_type,
                    bytes_base64: &encoded,
                },
                callback_url: &callback_url,
            })
            .send()
            .await?;
        anyhow::ensure!(
            response.status() == reqwest::StatusCode::ACCEPTED,
            "provider {plugin_id} returned {}",
            response.status()
        );
        Ok(())
    }
}

#[derive(Serialize)]
struct ProviderFile<'a> {
    media_type: &'a str,
    bytes_base64: &'a str,
}

/// What the worker posts to a private provider. `file` is the plugin's
/// `ResumeFile`. The callback body the provider sends back is `CallbackBody`.
#[derive(Serialize)]
struct ProviderJob<'a> {
    run_id: Uuid,
    file: ProviderFile<'a>,
    callback_url: &'a str,
}

pub fn plugin_secret() -> String {
    std::env::var("PLUGIN_SECRET").unwrap_or_else(|_| "spine-stub-secret".to_owned())
}

pub fn compiled_plugins() -> Vec<Arc<dyn Ats>> {
    plugins::compiled(plugin_secret())
}
