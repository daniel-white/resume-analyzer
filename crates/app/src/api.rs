//! Upload, list, detail, the live event stream, and the signed callback.
//! A resume belongs to the session from the magic-link cookie. Callbacks stay
//! on their own HMAC.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use domain::{Ats, CallbackBody, DOCX, MAX_UPLOAD_BYTES, PDF, SectionVote, verify};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use storage::{Objects, RecordOutcome};
use uuid::Uuid;

use crate::bus::Bus;

pub struct Api {
    pub store: storage::Store,
    pub objects: Arc<dyn Objects>,
    pub plugins: Vec<Arc<dyn Ats>>,
    pub bus: Bus,
    pub mail: mail::Smtp,
    pub auth_secret: String,
    pub app_origin: String,
    pub cookie_secure: bool,
    pub admin_email_domains: Vec<String>,
}

impl Api {
    fn plugin(&self, id: &str) -> Option<&Arc<dyn Ats>> {
        self.plugins.iter().find(|plugin| plugin.id() == id)
    }

    /// Exact domain match after `@`, case-insensitive. Empty configuration
    /// grants nobody admin access.
    pub fn is_admin_email(&self, email: &str) -> bool {
        let Some((_, domain)) = email.rsplit_once('@') else {
            return false;
        };
        let domain = domain.trim().to_ascii_lowercase();
        self.admin_email_domains
            .iter()
            .any(|allowed| allowed == &domain)
    }
}

#[derive(Serialize)]
struct Accepted {
    id: Uuid,
    version_id: Uuid,
    status: &'static str,
}

#[derive(Serialize)]
struct ResumeList {
    resumes: Vec<domain::ResumeSummary>,
}

#[derive(Serialize)]
struct PluginCard {
    id: &'static str,
    name: &'static str,
    subtitle: &'static str,
}

#[derive(Serialize)]
struct PluginList {
    plugins: Vec<PluginCard>,
}

#[derive(Serialize)]
struct CallbackStatus {
    status: &'static str,
}

pub async fn create_resume(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let user_id = match crate::auth_api::current_user(&api, &headers).await {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    match store_upload(&api, user_id, UploadKind::New, multipart).await {
        Ok(body) => (StatusCode::ACCEPTED, Json(body)).into_response(),
        Err(response) => *response,
    }
}

/// No account or public file URL is created. The random id is a short-lived
/// upload capability, passed only to the magic-link request.
pub async fn stage_resume(State(api): State<Arc<Api>>, mut multipart: Multipart) -> Response {
    let field = match next_file(&mut multipart).await {
        Ok(field) => field,
        Err(response) => return *response,
    };
    if field.name != "file" {
        return text(StatusCode::BAD_REQUEST, "missing file");
    }
    let Some(media_type) = sniff(&field.bytes) else {
        return text(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "That file is neither a PDF nor a DOCX.",
        );
    };
    let filename = field
        .filename
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| default_name(media_type).to_owned());
    let id = Uuid::new_v4();
    let key = format!("pending/{id}");
    if api.objects.put(&key, &field.bytes).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if api
        .store
        .stage_upload(id, &filename, media_type, &key)
        .await
        .is_err()
    {
        let _ = api.objects.delete(&key).await;
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    (StatusCode::ACCEPTED, Json(serde_json::json!({ "id": id }))).into_response()
}

pub async fn add_version(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(resume_id): Path<Uuid>,
    multipart: Multipart,
) -> Response {
    let user_id = match crate::auth_api::current_user(&api, &headers).await {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    match store_upload(&api, user_id, UploadKind::Existing(resume_id), multipart).await {
        Ok(body) => (StatusCode::ACCEPTED, Json(body)).into_response(),
        Err(response) => *response,
    }
}

pub async fn list_resumes(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let user_id = match crate::auth_api::current_user(&api, &headers).await {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    match api.store.list(user_id).await {
        Ok(resumes) => Json(ResumeList { resumes }).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct RenameResume {
    title: String,
}

pub async fn rename_resume(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<RenameResume>,
) -> Response {
    let user_id = match crate::auth_api::current_user(&api, &headers).await {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    let title = body.title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return text(
            StatusCode::BAD_REQUEST,
            "Use a name between 1 and 200 characters.",
        );
    }
    match api.store.rename_resume(user_id, id, title).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn resume_detail(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    let user_id = match crate::auth_api::current_user(&api, &headers).await {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    match api.store.detail(user_id, id).await {
        Ok(Some(detail)) => {
            // The legend can grow after this resume was completed. Queue only
            // missing runs on the displayed version; fanout preserves existing
            // results and atomically writes each run with its outbox event.
            if let Some(version) = detail.versions.first()
                && version.parse.is_some()
                && api
                    .plugins
                    .iter()
                    .any(|plugin| !version.runs.iter().any(|run| run.plugin_id == plugin.id()))
            {
                let ids: Vec<&str> = api.plugins.iter().map(|plugin| plugin.id()).collect();
                if api.store.fanout(version.id, &ids).await.is_err() {
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
                return match api.store.detail(user_id, id).await {
                    Ok(Some(detail)) => Json(detail).into_response(),
                    Ok(None) => StatusCode::NOT_FOUND.into_response(),
                    Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                };
            }
            Json(detail).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Original bytes, never a public object URL. Unknown and unowned files look alike.
pub async fn resume_file(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path((resume_id, version_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let user_id = match crate::auth_api::current_user(&api, &headers).await {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    let file = match api.store.version_file(user_id, resume_id, version_id).await {
        Ok(Some(file)) => file,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let bytes = match api.objects.get(&file.object_key).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let filename = default_name(&file.media_type);
    (
        [
            (header::CONTENT_TYPE, file.media_type),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{filename}\""),
            ),
            (header::CACHE_CONTROL, "private, no-store".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        bytes,
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct SectionVoteBody {
    /// `null` clears the vote.
    vote: Option<SectionVote>,
}

#[derive(Serialize)]
struct SectionVoteResponse {
    vote: Option<SectionVote>,
}

/// Set or clear the caller's vote on one detected section. Section ids are
/// stable within a version, and unknown or unowned ids are indistinguishable
/// from each other.
pub async fn vote_section(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path((resume_id, version_id, section_id)): Path<(Uuid, Uuid, String)>,
    Json(body): Json<SectionVoteBody>,
) -> Response {
    let user_id = match crate::auth_api::current_user(&api, &headers).await {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    match api
        .store
        .vote_section(user_id, resume_id, version_id, &section_id, body.vote)
        .await
    {
        Ok(true) => Json(SectionVoteResponse { vote: body.vote }).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            eprintln!("vote section: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn plugins(State(api): State<Arc<Api>>) -> Response {
    Json(PluginList {
        plugins: api
            .plugins
            .iter()
            .map(|plugin| PluginCard {
                id: plugin.id(),
                name: plugin.name(),
                subtitle: plugin.subtitle(),
            })
            .collect(),
    })
    .into_response()
}

pub async fn delete_resume(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    delete_owned(&api, &headers, id, None).await
}

pub async fn delete_version(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path((id, version_id)): Path<(Uuid, Uuid)>,
) -> Response {
    delete_owned(&api, &headers, id, Some(version_id)).await
}

async fn delete_owned(
    api: &Api,
    headers: &HeaderMap,
    id: Uuid,
    version_id: Option<Uuid>,
) -> Response {
    let user_id = match crate::auth_api::current_user(api, headers).await {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    let removed = match api.store.delete_resume(user_id, id, version_id).await {
        Ok(Some(removed)) => removed,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            eprintln!("delete resume: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    for version in removed {
        if let Err(err) = api.objects.delete(&version.object_key).await {
            eprintln!("delete object {}: {err}", version.object_key);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

/// Live only. The receiver is created when the request arrives, so events
/// that were already dispatched are not replayed.
pub async fn events(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, StatusCode> {
    let user_id = crate::auth_api::current_user(&api, &headers).await?;
    if api
        .store
        .detail(user_id, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .is_none()
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let mut incoming = api.bus.subscribe(id);
    let stream = async_stream::stream! {
        loop {
            match incoming.recv().await {
                Ok(event) => {
                    let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_owned());
                    yield Ok(
                        Event::default()
                            .event(event.name.as_str())
                            .id(event.id.to_string())
                            .data(data),
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

pub async fn callback(
    State(api): State<Arc<Api>>,
    Path(plugin_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(plugin) = api.plugin(&plugin_id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(timestamp) = header_str(&headers, "x-timestamp").and_then(|value| value.parse().ok())
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(signature) = header_str(&headers, "x-signature") else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(key) = header_str(&headers, "idempotency-key") else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if verify(
        plugin.secret(),
        chrono::Utc::now().timestamp(),
        timestamp,
        &body,
        signature,
    )
    .is_err()
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(parsed) = serde_json::from_slice::<CallbackBody>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match api
        .store
        .record_callback(
            &plugin_id,
            parsed.run_id,
            key,
            &parsed.parse,
            &parsed.verdict,
        )
        .await
    {
        Ok(RecordOutcome::Recorded) => {
            (StatusCode::OK, Json(CallbackStatus { status: "recorded" })).into_response()
        }
        Ok(RecordOutcome::Replay) => {
            (StatusCode::OK, Json(CallbackStatus { status: "replay" })).into_response()
        }
        Ok(RecordOutcome::UnknownRun) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

enum UploadKind {
    New,
    Existing(Uuid),
}

async fn store_upload(
    api: &Api,
    user_id: Uuid,
    kind: UploadKind,
    mut multipart: Multipart,
) -> Result<Accepted, Box<Response>> {
    let field = next_file(&mut multipart).await?;
    if field.name != "file" {
        return Err(Box::new(text(StatusCode::BAD_REQUEST, "missing file")));
    }
    let Some(media_type) = sniff(&field.bytes) else {
        return Err(Box::new(text(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "file is neither a PDF nor a DOCX",
        )));
    };
    let filename = field
        .filename
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| default_name(media_type).to_owned());
    let version_id = Uuid::new_v4();
    let resume_id = match kind {
        UploadKind::New => Uuid::new_v4(),
        UploadKind::Existing(id) => {
            let known = api
                .store
                .detail(user_id, id)
                .await
                .map_err(|_| Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()))?;
            if known.is_none() {
                return Err(Box::new(StatusCode::NOT_FOUND.into_response()));
            }
            id
        }
    };
    let object_key = domain::version_object_key(user_id, resume_id, version_id);
    api.objects
        .put(&object_key, &field.bytes)
        .await
        .map_err(|_| Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()))?;
    match kind {
        UploadKind::New => {
            api.store
                .create_resume(
                    user_id,
                    resume_id,
                    version_id,
                    &filename,
                    media_type,
                    &object_key,
                )
                .await
                .map_err(|_| Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()))?;
        }
        UploadKind::Existing(_) => {
            let number = api
                .store
                .add_version(
                    user_id,
                    resume_id,
                    version_id,
                    &filename,
                    media_type,
                    &object_key,
                )
                .await
                .map_err(|_| Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()))?;
            if number.is_none() {
                return Err(Box::new(StatusCode::NOT_FOUND.into_response()));
            }
        }
    }
    Ok(Accepted {
        id: resume_id,
        version_id,
        status: "uploaded",
    })
}

struct Upload {
    name: String,
    filename: Option<String>,
    bytes: Vec<u8>,
}

async fn next_file(multipart: &mut Multipart) -> Result<Upload, Box<Response>> {
    let field = multipart
        .next_field()
        .await
        .map_err(|_| Box::new(text(StatusCode::BAD_REQUEST, "missing file")))?
        .ok_or_else(|| Box::new(text(StatusCode::BAD_REQUEST, "missing file")))?;
    let name = field.name().unwrap_or_default().to_owned();
    let filename = field.file_name().map(str::to_owned);
    let bytes = field
        .bytes()
        .await
        .map_err(|_| Box::new(text(StatusCode::BAD_REQUEST, "missing file")))?;
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(Box::new(text(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file is over 5 MiB",
        )));
    }
    Ok(Upload {
        name,
        filename,
        bytes: bytes.to_vec(),
    })
}

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"%PDF") {
        return Some(PDF);
    }
    if is_docx(bytes) {
        return Some(DOCX);
    }
    None
}

fn is_docx(bytes: &[u8]) -> bool {
    let cursor = std::io::Cursor::new(bytes);
    let Ok(mut archive) = zip::ZipArchive::new(cursor) else {
        return false;
    };
    archive.by_name("word/document.xml").is_ok()
}

fn default_name(media_type: &str) -> &'static str {
    if media_type == PDF {
        "resume.pdf"
    } else {
        "resume.docx"
    }
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

fn text(status: StatusCode, message: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain")],
        message.to_owned(),
    )
        .into_response()
}
