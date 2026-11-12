//! Drain the outbox in this process. The compiled stub is called with the raw
//! file and the webhook URL. A fleet plugin is handed to its Python provider,
//! which posts the callback itself. Only that callback is stored.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use domain::{
    AnalyzeFeedbackPayload, Ats, EventName, EventRecord, GenerateMarkdownPayload, ResumeFile,
    RunPayload, UploadedPayload,
};
use feedback::{markdown, FeedbackAnalyzer};
use plugins::structure;
use sections::SectionDetector;
use serde::Serialize;
use storage::{Objects, Store};
use summary::SummaryGenerator;
use tracing::{Level, field, info, span};
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
    notifications: Option<(mail::Smtp, String)>,
    detector: SectionDetector,
    summarizer: SummaryGenerator,
    feedback: FeedbackAnalyzer,
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
            notifications: None,
            detector: SectionDetector::heuristic(),
            summarizer: SummaryGenerator::from_env(),
            feedback: FeedbackAnalyzer::from_env(),
        }
    }

    /// The section detector the worker uses. The default is the offline
    /// heading-alias engine so tests never download a model.
    pub fn detector(mut self, detector: SectionDetector) -> Self {
        self.detector = detector;
        self
    }

    /// The profile summary generator. Tests default to the offline extractor.
    pub fn summarizer(mut self, summarizer: SummaryGenerator) -> Self {
        self.summarizer = summarizer;
        self
    }

    /// The feedback analyzer that diagnoses thumbs downs. Tests default to the
    /// offline extractor.
    pub fn feedback(mut self, feedback: FeedbackAnalyzer) -> Self {
        self.feedback = feedback;
        self
    }

    /// Python provider urls, keyed by plugin id. Empty keeps the in-process stub.
    pub fn endpoints(mut self, endpoints: HashMap<String, String>) -> Self {
        self.endpoints = endpoints;
        self
    }

    pub fn notifications(mut self, smtp: mail::Smtp, app_origin: impl Into<String>) -> Self {
        self.notifications = Some((smtp, app_origin.into()));
        self
    }

    /// Keep claiming until the outbox is idle. Used by the upload flow test so an
    /// upload can reach a callback without a background loop.
    pub async fn drain(&self) -> anyhow::Result<()> {
        while self.step().await? {}
        Ok(())
    }

    pub fn spawn(self: Arc<Self>) {
        if self.notifications.is_some() {
            let worker = self.clone();
            tokio::spawn(async move {
                loop {
                    match worker.send_analysis_batch().await {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(err) => eprintln!("analysis mail: {err:#}"),
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
        }
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
            EventName::GenerateMarkdown => self.generate_markdown(event).await,
            EventName::AnalyzeFeedback => self.analyze_feedback(event).await,
        }
    }

    /// Mail runs separately so its batching window and retries never delay SSE.
    pub async fn send_analysis_batch(&self) -> anyhow::Result<bool> {
        let Some((smtp, origin)) = &self.notifications else {
            return Ok(false);
        };
        let Some(batch) = self.store.claim_analysis_batch().await? else {
            return Ok(false);
        };
        let result = async {
            let mut results = Vec::new();
            for (_, plugin_id, verdict) in &batch.runs {
                let provider = self
                    .plugins
                    .iter()
                    .find(|plugin| plugin.id() == plugin_id)
                    .map(|plugin| plugin.name())
                    .unwrap_or(plugin_id);
                let mut url = reqwest::Url::parse(origin)?;
                url.set_path(&format!("/resumes/{}", batch.resume_id));
                url.set_query(None);
                url.set_fragment(None);
                url.query_pairs_mut()
                    .append_pair("version", &batch.version_id.to_string())
                    .append_pair("provider", plugin_id);
                results.push(mail::AnalysisResult {
                    provider: provider.to_owned(),
                    score: verdict.score,
                    url: url.into(),
                });
            }
            smtp.send_analysis_done(&batch.email, &batch.filename, batch.number, &results)
                .await?;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        self.store
            .finish_analysis_batch(&batch, result.is_ok())
            .await?;
        result?;
        Ok(true)
    }

    async fn uploaded(&self, event: &EventRecord) -> anyhow::Result<()> {
        let payload: UploadedPayload = serde_json::from_value(event.payload.clone())?;
        let bytes = self.objects.get(&payload.object_key).await?;
        let file = ResumeFile {
            media_type: &payload.media_type,
            bytes: &bytes,
        };
        let parse = structure(&file);
        // Reading lines and running the model is CPU work; keep the event loop
        // free while a cold model loads.
        let lines = plugins::document_lines(&file);
        let detector = self.detector.clone();
        let summarizer = self.summarizer.clone();
        let summary_parse = parse.clone();
        // One span per upload, so every summary.chat span below is attributable
        // to the resume and version that triggered it.
        let span = span!(
            Level::INFO,
            "resume.upload.summary",
            resume_id = %event.resume_id,
            version_id = %event.version_id,
            engine = field::Empty,
            model = field::Empty,
        );
        let _enter = span.enter();
        let (sections, profile_summary) = tokio::task::spawn_blocking(move || {
            let sections = detector.detect_document(&lines);
            let summary = summarizer.generate(&summary_parse, &sections)?;
            Ok::<_, anyhow::Error>((sections, summary))
        })
        .await?
        .map_err(|err| anyhow::anyhow!("summary generation failed: {err}"))?;
        span.record("engine", &profile_summary.engine);
        span.record("model", &profile_summary.model);
        info!("profile summary generated");
        self.store
            .save_parse(
                event.version_id,
                &parse,
                Some(&sections),
                Some(&profile_summary),
            )
            .await
    }

    async fn parsed(&self, event: &EventRecord) -> anyhow::Result<()> {
        let ids: Vec<&str> = self.plugins.iter().map(|plugin| plugin.id()).collect();
        self.store.fanout(event.version_id, &ids).await
    }

    /// Build the markdown rendering once, then diagnose every down vote that
    /// was waiting for it.
    async fn generate_markdown(&self, event: &EventRecord) -> anyhow::Result<()> {
        let payload: GenerateMarkdownPayload = serde_json::from_value(event.payload.clone())?;
        let Some((parse, sections)) = self.store.parse_and_sections(payload.version_id).await?
        else {
            return Ok(());
        };
        let markdown =
            tokio::task::spawn_blocking(move || markdown(&parse, &sections)).await?;
        self.store
            .save_resume_markdown(payload.version_id, &markdown)
            .await?;
        self.store
            .enqueue_feedback_analyses(payload.version_id)
            .await
    }

    /// Compare the stored markdown with the original resume and store the
    /// model's diagnosis of one rejected reading.
    async fn analyze_feedback(&self, event: &EventRecord) -> anyhow::Result<()> {
        let payload: AnalyzeFeedbackPayload = serde_json::from_value(event.payload.clone())?;
        let Some(markdown) = self.store.resume_markdown(payload.version_id).await? else {
            // The markdown build has not run yet. Fail so the event is released
            // and retried after it exists instead of diagnosing nothing.
            anyhow::bail!("markdown not ready for version {}", payload.version_id);
        };
        let (object_key, media_type) = self.store.file_of(payload.version_id).await?;
        let bytes = self.objects.get(&object_key).await?;
        let original = plugins::document_lines(&ResumeFile {
            media_type: &media_type,
            bytes: &bytes,
        })
        .into_iter()
        .map(|line| line.text)
        .collect::<Vec<_>>()
        .join("\n");
        let analyzer = self.feedback.clone();
        let section_id = payload.section_id.clone();
        let heading = payload.section_heading.clone();
        let kind = payload.section_kind;
        // One span per diagnosis, so the feedback.chat span below is
        // attributable to the version and section that triggered it.
        let span = span!(
            Level::INFO,
            "resume.feedback",
            version_id = %payload.version_id,
            section_id = %section_id,
            engine = field::Empty,
            model = field::Empty,
        );
        let _enter = span.enter();
        let diagnosis_result = tokio::task::spawn_blocking(move || {
            analyzer.analyze(&markdown.text, &original, &section_id, &heading, kind)
        })
        .await??;
        span.record("engine", &diagnosis_result.engine);
        span.record("model", &diagnosis_result.model);
        info!("feedback diagnosis generated");
        self.store
            .save_feedback_analysis(
                &payload,
                &diagnosis_result.engine,
                &diagnosis_result.model,
                &diagnosis_result.text,
            )
            .await?;
        Ok(())
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
    std::env::var("PLUGIN_SECRET").unwrap_or_else(|_| "demo-stub-secret".to_owned())
}

pub fn compiled_plugins() -> Vec<Arc<dyn Ats>> {
    plugins::compiled(plugin_secret())
}
