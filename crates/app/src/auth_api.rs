//! Magic-link login, the session cookie, and account deletion.
//! The browser calls these on the same origin. The cookie is `HttpOnly` and
//! `SameSite=Lax`, so a later `/api` request carries it.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use serde::{Deserialize, Serialize};

use crate::api::Api;

#[derive(Deserialize)]
struct EmailBody {
    email: String,
    return_to: Option<String>,
    upload_id: Option<auth::Uuid>,
    /// Browser-reported IANA timezone, e.g. "America/New_York". Absent or
    /// unparseable values are stored as NULL and fall back to UTC.
    timezone: Option<String>,
}

#[derive(Deserialize)]
struct TokenBody {
    token: String,
}

#[derive(Serialize)]
struct IdentityBody {
    id: auth::Uuid,
    email: String,
}

#[derive(Serialize)]
struct MeBody {
    id: auth::Uuid,
    email: String,
    admin: bool,
    /// The session's timezone, if the browser reported one at sign-in.
    timezone: Option<String>,
    sessions: Vec<SessionBody>,
}

#[derive(Serialize)]
struct SessionBody {
    id: auth::Uuid,
    #[serde(with = "time::serde::rfc3339")]
    created_at: auth::OffsetDateTime,
    current: bool,
}

pub fn routes() -> axum::Router<Arc<Api>> {
    axum::Router::new()
        .route("/api/auth/magic-link", post(request_magic_link))
        .route("/api/auth/magic-link/verify", post(verify_magic_link))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/sessions/{id}", delete(end_session))
        .route("/api/me", get(me).delete(delete_account))
}

/// The signed-in user, or the status to send back.
pub async fn current_user(api: &Api, headers: &HeaderMap) -> Result<auth::Uuid, StatusCode> {
    current_session(api, headers)
        .await
        .map(|session| session.user_id)
}

/// Signed in and on a configured admin domain. Signed-out callers get 401;
/// signed-in callers on any other domain get 403.
pub async fn current_admin(api: &Api, headers: &HeaderMap) -> Result<auth::Session, StatusCode> {
    let session = current_session(api, headers).await?;
    if api.is_admin_email(&session.email) {
        Ok(session)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

async fn request_magic_link(State(api): State<Arc<Api>>, Json(body): Json<EmailBody>) -> Response {
    let now = auth::OffsetDateTime::now_utc();
    let issued = match auth::request_magic_link(
        api.store.pool(),
        &body.email,
        now,
        validate_timezone(body.timezone.as_deref()),
    )
    .await
    {
        Ok(issued) => issued,
        Err(auth::AuthError::Email) => return error(StatusCode::BAD_REQUEST, "email"),
        Err(err) => {
            eprintln!("magic link: {err}");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable");
        }
    };
    if let Some(id) = body.upload_id {
        match auth::bind_upload(api.store.pool(), &issued.token, id).await {
            Ok(true) => {}
            Ok(false) => return error(StatusCode::BAD_REQUEST, "upload"),
            Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable"),
        }
    }
    let origin = api.app_origin.trim_end_matches('/');
    let mut url = match reqwest::Url::parse(&format!("{origin}/login/verify")) {
        Ok(url) => url,
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable"),
    };
    url.query_pairs_mut().append_pair("token", &issued.token);
    if let Some(return_to) = body.return_to.filter(|path| valid_return_to(path)) {
        url.query_pairs_mut().append_pair("returnTo", &return_to);
    }
    if let Err(err) = api.mail.send_magic_link(&issued.email, url.as_str()).await {
        eprintln!("magic link mail: {err}");
        return error(StatusCode::BAD_GATEWAY, "mail");
    }
    StatusCode::ACCEPTED.into_response()
}

fn valid_return_to(path: &str) -> bool {
    if path == "/admin" {
        return true;
    }
    let Some(id) = path.strip_prefix("/resumes/") else {
        return false;
    };
    let id = id.split('?').next().unwrap_or_default();
    auth::Uuid::parse_str(id).is_ok() && !path.contains(['\\', '\r', '\n', '#'])
}

async fn verify_magic_link(State(api): State<Arc<Api>>, Json(body): Json<TokenBody>) -> Response {
    let now = auth::OffsetDateTime::now_utc();
    let session = match auth::verify_magic_link(api.store.pool(), &body.token, now).await {
        Ok(session) => session,
        Err(auth::AuthError::Link) => return error(StatusCode::BAD_REQUEST, "link"),
        Err(err) => {
            eprintln!("magic link verify: {err}");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable");
        }
    };
    let cookie = session_cookie(
        &auth::seal_session(&api.auth_secret, session.id),
        auth::session_max_age_secs(),
        api.cookie_secure,
    );
    let body = Json(IdentityBody {
        id: session.user_id,
        email: session.email,
    });
    with_cookie((StatusCode::OK, body).into_response(), cookie)
}

async fn me(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let current = match current_session(&api, &headers).await {
        Ok(session) => session,
        Err(StatusCode::UNAUTHORIZED) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(status) => return error(status, "unavailable"),
    };
    let listed = match auth::list_sessions(
        api.store.pool(),
        current.user_id,
        auth::OffsetDateTime::now_utc(),
    )
    .await
    {
        Ok(listed) => listed,
        Err(err) => {
            eprintln!("sessions: {err}");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable");
        }
    };
    let mut sessions: Vec<SessionBody> = listed
        .into_iter()
        .map(|session| SessionBody {
            current: session.id == current.id,
            id: session.id,
            created_at: session.created_at,
        })
        .collect();
    sessions.sort_by_key(|session| std::cmp::Reverse(session.current));
    let admin = api.is_admin_email(&current.email);
    Json(MeBody {
        id: current.user_id,
        email: current.email,
        admin,
        timezone: current.timezone,
        sessions,
    })
    .into_response()
}

async fn logout(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    match current_session(&api, &headers).await {
        Ok(session) => {
            if let Err(err) = auth::end_session(api.store.pool(), session.user_id, session.id).await
            {
                eprintln!("logout: {err}");
                return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable");
            }
        }
        Err(StatusCode::UNAUTHORIZED) => {}
        Err(status) => return error(status, "unavailable"),
    }
    clear_session(&api)
}

async fn end_session(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<auth::Uuid>,
) -> Response {
    let current = match current_session(&api, &headers).await {
        Ok(session) => session,
        Err(StatusCode::UNAUTHORIZED) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(status) => return error(status, "unavailable"),
    };
    let ended = match auth::end_session(api.store.pool(), current.user_id, id).await {
        Ok(ended) => ended,
        Err(err) => {
            eprintln!("end session: {err}");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable");
        }
    };
    if !ended {
        return error(StatusCode::NOT_FOUND, "session");
    }
    if id == current.id {
        clear_session(&api)
    } else {
        StatusCode::NO_CONTENT.into_response()
    }
}

async fn delete_account(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let current = match current_session(&api, &headers).await {
        Ok(session) => session,
        Err(StatusCode::UNAUTHORIZED) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(status) => return error(status, "unavailable"),
    };
    let removed = match api.store.delete_user(current.user_id).await {
        Ok(removed) => removed,
        Err(err) => {
            eprintln!("delete resumes: {err}");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable");
        }
    };
    for version in &removed {
        if let Err(err) = api.objects.delete(&version.object_key).await {
            eprintln!("delete object {}: {err}", version.object_key);
            return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable");
        }
    }
    if let Err(err) = auth::delete_account(api.store.pool(), current.user_id).await {
        eprintln!("delete account: {err}");
        return error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable");
    }
    clear_session(&api)
}

async fn current_session(api: &Api, headers: &HeaderMap) -> Result<auth::Session, StatusCode> {
    let Some(raw) = read_cookie(headers) else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let Some(id) = auth::open_session(&api.auth_secret, raw) else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    match auth::find_session(api.store.pool(), id, auth::OffsetDateTime::now_utc()).await {
        Ok(Some(session)) => Ok(session),
        Ok(None) => Err(StatusCode::UNAUTHORIZED),
        Err(err) => {
            eprintln!("session: {err}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn read_cookie(headers: &HeaderMap) -> Option<&str> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == "session" && !value.is_empty()).then_some(value)
    })
}

fn session_cookie(value: &str, max_age: i64, secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    let header =
        format!("session={value}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age}{secure}");
    HeaderValue::from_str(&header).expect("session cookie is ascii")
}

fn clear_session(api: &Api) -> Response {
    let cookie = session_cookie("", 0, api.cookie_secure);
    with_cookie(StatusCode::NO_CONTENT.into_response(), cookie)
}

fn with_cookie(mut response: Response, cookie: HeaderValue) -> Response {
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    response
}

fn error(status: StatusCode, code: &'static str) -> Response {
    (status, Json(serde_json::json!({ "error": code }))).into_response()
}

/// Keep only IANA names the browser could plausibly report. The client already
/// validated the value with `Intl.DateTimeFormat`, so this is a cheap defense
/// against junk in transit: non-empty, bounded length, no control characters.
/// Unknown names are dropped rather than trusted, and the client falls back
/// to UTC for them.
fn validate_timezone(timezone: Option<&str>) -> Option<&str> {
    let timezone = timezone?.trim();
    if timezone.is_empty() || timezone.len() > 64 || timezone.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(timezone)
}

#[cfg(test)]
mod return_to_tests {
    use super::valid_return_to;

    #[test]
    fn accepts_only_local_resume_and_admin_destinations() {
        let id = "12345678-1234-1234-1234-123456789abc";
        assert!(valid_return_to(&format!(
            "/resumes/{id}?version={id}&provider=greenline"
        )));
        assert!(valid_return_to("/admin"));
        for path in [
            "https://evil.example",
            "//evil.example",
            "/login",
            "/resumes/not-an-id",
            "/admin/",
            "/admin?tab=open",
        ] {
            assert!(!valid_return_to(path));
        }
        assert!(!valid_return_to(&format!("/resumes/{id}\\evil")));
    }
}
