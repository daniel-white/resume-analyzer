mod config;
mod health;

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::get;

use crate::config::Config;
use crate::health::Report;

#[derive(Clone)]
struct AppState {
    config: Config,
    s3: aws_sdk_s3::Client,
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_route))
        .with_state(Arc::new(state))
}

async fn health_route(
    State(state): State<Arc<AppState>>,
) -> (axum::http::StatusCode, Json<Report>) {
    let report = health::report_for(&state.config, &state.s3).await;
    (report.http_status(), Json(report))
}

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    let bind = config.bind.clone();
    let s3 = health::s3_client(&config).await;
    let state = AppState { config, s3 };
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|err| panic!("bind {bind}: {err}"));
    eprintln!("listening on {bind}");
    axum::serve(listener, router(state)).await.expect("server");
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_is_unavailable_when_nothing_is_listening() {
        let config = Config::unreachable();
        let s3 = health::s3_client(&config).await;
        let app = router(AppState { config, s3 });
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["checks"]["postgres"], "error");
        assert_eq!(json["checks"]["s3"], "error");
        assert_eq!(json["checks"]["smtp"], "error");
    }
}
