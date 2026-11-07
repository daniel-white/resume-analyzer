//! HTTP server, the outbox worker, and the live event stream.

mod api;
mod auth_api;
mod bus;
mod config;
mod health;
mod objects;
mod providers;
mod worker;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, State};
use axum::routing::{delete, get, post};
use domain::MAX_UPLOAD_BYTES;

use crate::config::Config;
use crate::health::Report;
use crate::objects::S3Objects;

pub use crate::api::Api;
pub use crate::bus::Bus;
pub use crate::worker::{Worker, compiled_plugins};

#[derive(Clone)]
struct HealthState {
    config: Config,
    s3: aws_sdk_s3::Client,
}

fn health_router(state: HealthState) -> Router {
    Router::new()
        .route("/health", get(health_route))
        .with_state(Arc::new(state))
}

/// Routes the spine test drives. Health stays on its own router so a missing
/// database does not block the liveness probe.
pub fn api_router(api: Arc<Api>) -> Router {
    Router::new()
        .route(
            "/api/resumes",
            get(api::list_resumes).post(api::create_resume),
        )
        .route(
            "/api/resumes/{id}",
            get(api::resume_detail).delete(api::delete_resume),
        )
        .route("/api/resumes/{id}/versions", post(api::add_version))
        .route(
            "/api/resumes/{id}/versions/{version_id}/file",
            get(api::resume_file),
        )
        .route(
            "/api/resumes/{id}/versions/{version_id}",
            delete(api::delete_version),
        )
        .route("/api/resumes/{id}/events", get(api::events))
        .route("/api/plugins", get(api::plugins))
        .route("/api/uploads", post(api::stage_resume))
        .route("/api/callbacks/{plugin_id}", post(api::callback))
        .merge(auth_api::routes())
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES + 256 * 1024))
        .with_state(api)
}

fn router(health: HealthState, api: Arc<Api>) -> Router {
    health_router(health).merge(api_router(api))
}

async fn health_route(
    State(state): State<Arc<HealthState>>,
) -> (axum::http::StatusCode, Json<Report>) {
    let report = health::report_for(&state.config, &state.s3).await;
    (report.http_status(), Json(report))
}

/// `0.0.0.0` is not a place the worker can post a callback.
pub fn callback_url(address: SocketAddr) -> String {
    let ip = if address.ip().is_unspecified() {
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    } else {
        address.ip()
    };
    format!("http://{ip}:{}", address.port())
}

pub async fn serve() {
    let config = Config::from_env();
    if config.auth_secret.len() < 16 {
        panic!("AUTH_SECRET must be at least 16 characters");
    }
    let bind = config.bind.clone();
    let s3 = health::s3_client(&config).await;
    let store = storage::Store::connect(&config.database_url)
        .await
        .unwrap_or_else(|err| panic!("postgres: {err}"));
    store
        .migrate()
        .await
        .unwrap_or_else(|err| panic!("migrate: {err}"));
    let objects = Arc::new(S3Objects::new(s3.clone(), config.s3_bucket.clone()));
    let fleet = providers::fleet_from_env(worker::plugin_secret())
        .unwrap_or_else(|err| panic!("ats providers: {err}"));
    for (id, url) in &fleet.endpoints {
        eprintln!("ats {id} {url}");
    }
    let plugins = fleet.plugins;
    let bus = Bus::default();
    let api = Arc::new(Api {
        store: store.clone(),
        objects: objects.clone(),
        plugins: plugins.clone(),
        bus: bus.clone(),
        mail: mail::Smtp::new(&config.smtp_host, config.smtp_port),
        auth_secret: config.auth_secret.clone(),
        app_origin: config.app_origin.clone(),
        cookie_secure: config.cookie_secure(),
    });
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|err| panic!("bind {bind}: {err}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|err| panic!("local address: {err}"));
    let callback = std::env::var("CALLBACK_URL")
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_owned())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| callback_url(address));
    let worker = Arc::new(
        Worker::new(store, objects, plugins, bus, callback)
            .endpoints(fleet.endpoints)
            .notifications(api.mail.clone(), &config.app_origin),
    );
    worker.spawn();
    let cleanup_api = api.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(err) = cleanup_api
                .store
                .clean_pending_uploads(cleanup_api.objects.as_ref())
                .await
            {
                eprintln!("pending upload cleanup: {err}");
            }
        }
    });
    let health = HealthState { config, s3 };
    eprintln!("listening on {address}");
    axum::serve(listener, router(health, api))
        .await
        .expect("server");
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::{HealthState, health_router};
    use crate::config::Config;
    use crate::health;

    #[tokio::test]
    async fn health_is_unavailable_when_nothing_is_listening() {
        let config = Config::unreachable();
        let s3 = health::s3_client(&config).await;
        let response = health_router(HealthState { config, s3 })
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
