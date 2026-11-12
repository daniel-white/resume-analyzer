//! Admin-only feedback triage. Same session cookie as the rest of the API;
//! authorization is an email domain from configuration.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use serde::{Deserialize, Serialize};
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
