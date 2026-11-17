//! Admin-only feedback triage. Same session cookie as the rest of the API;
//! authorization is an email domain from configuration.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use domain::EventName;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::api::Api;

#[derive(Serialize)]
struct AdminFeedbackList {
    feedback: Vec<domain::AdminFeedback>,
}

#[derive(Deserialize)]
struct HandledBody {
    handled: bool,
}

pub fn routes() -> axum::Router<Arc<Api>> {
    axum::Router::new()
        .route("/api/admin/feedback", get(list_feedback))
        .route(
            "/api/admin/feedback/analyses/{id}",
            put(set_analysis_handled),
        )
        .route("/api/admin/feedback/analyses/{id}", post(rerun_analysis))
        .route("/api/admin/events", get(events))
}

/// Admin-only view of diagnoses as they are generated. Admins do not own the
/// resumes they review, so this uses the global generation channel rather than
/// the per-resume owner stream. Only feedback frames are forwarded.
pub async fn events(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    crate::auth_api::current_admin(&api, &headers).await?;
    let mut incoming = api.bus.subscribe_stream();
    let stream = async_stream::stream! {
        loop {
            match incoming.recv().await {
                Ok(event) => {
                    if event.name != EventName::GenerationStream {
                        continue;
                    }
                    let flow = event.payload.get("flow").and_then(|value| value.as_str());
                    if flow != Some("feedback") {
                        continue;
                    }
                    let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_owned());
                    yield Ok(
                        Event::default()
                            .event(event.name.as_str())
                            .id(event.id.to_string())
                            .data(data),
                    );
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

async fn list_feedback(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    if let Err(status) = crate::auth_api::current_admin(&api, &headers).await {
        return status.into_response();
    }
    match api.store.admin_feedback().await {
        Ok(feedback) => Json(AdminFeedbackList { feedback }).into_response(),
        Err(err) => {
            eprintln!("admin feedback: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn rerun_analysis(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(status) = crate::auth_api::current_admin(&api, &headers).await {
        return status.into_response();
    }
    match api.store.rerun_feedback_analysis(id).await {
        Ok(true) => StatusCode::ACCEPTED.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            eprintln!("admin feedback rerun: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn set_analysis_handled(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<HandledBody>,
) -> Response {
    let admin = match crate::auth_api::current_admin(&api, &headers).await {
        Ok(admin) => admin,
        Err(status) => return status.into_response(),
    };
    let handled_by = body.handled.then_some(admin.email.as_str());
    match api
        .store
        .set_feedback_analysis_handled(id, handled_by)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            eprintln!("admin feedback handle: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
