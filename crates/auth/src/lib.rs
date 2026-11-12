//! Magic-link sign-in and the server-side sessions that follow it.
//!
//! A link stores only the SHA-256 of a 32-byte token. The cookie is the
//! session id plus an HMAC, and the session row is what makes it revocable.

mod store;

use hmac::{KeyInit, Mac};
use sha2::Digest;

pub use store::{IssuedLink, ListedSession, Session};
pub use time::OffsetDateTime;
pub use uuid::Uuid;

const MAGIC_LINK: time::Duration = time::Duration::minutes(15);
const SESSION: time::Duration = time::Duration::days(14);

#[derive(Debug)]
pub enum AuthError {
    Email,
    Link,
    Database(sqlx::Error),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Email => write!(f, "invalid email"),
            Self::Link => write!(f, "invalid magic link"),
            Self::Database(err) => write!(f, "database: {err}"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<sqlx::Error> for AuthError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err)
    }
}

pub fn session_max_age_secs() -> i64 {
    SESSION.whole_seconds()
}

pub fn normalize_email(input: &str) -> Option<String> {
    let email = input.trim().to_lowercase();
    if email.len() > 320 || email.chars().any(char::is_whitespace) {
        return None;
    }
    let (local, domain) = email.split_once('@')?;
    if local.is_empty()
        || domain.contains('@')
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return None;
    }
    Some(email)
}

pub fn seal_session(secret: &str, session_id: Uuid) -> String {
    let signature = sign(secret, session_id);
    format!("{session_id}.{signature}")
}

pub fn open_session(secret: &str, cookie: &str) -> Option<Uuid> {
    let (id_part, signature) = cookie.split_once('.')?;
    let session_id = Uuid::parse_str(id_part).ok()?;
    let expected = sign(secret, session_id);
    if signature.len() != expected.len() {
        return None;
    }
    let mut mismatch = 0_u8;
    for (left, right) in signature.bytes().zip(expected.bytes()) {
        mismatch |= left ^ right;
    }
    (mismatch == 0).then_some(session_id)
}

pub async fn request_magic_link(
    pool: &sqlx::PgPool,
    email: &str,
    now: OffsetDateTime,
) -> Result<IssuedLink, AuthError> {
    let email = normalize_email(email).ok_or(AuthError::Email)?;
    let token = random_token();
    store::insert_magic_link(pool, &email, &token, now, now + MAGIC_LINK).await?;
    Ok(IssuedLink {
        token: encode_token(token),
        email,
        expires_at: now + MAGIC_LINK,
    })
}

pub async fn verify_magic_link(
    pool: &sqlx::PgPool,
    token: &str,
    now: OffsetDateTime,
) -> Result<Session, AuthError> {
    let Some(raw) = decode_token(token) else {
        return Err(AuthError::Link);
    };
    store::consume_magic_link(pool, &raw, now, now + SESSION).await
}

pub async fn bind_upload(
    pool: &sqlx::PgPool,
    token: &str,
    upload_id: Uuid,
) -> Result<bool, AuthError> {
    let Some(raw) = decode_token(token) else {
        return Err(AuthError::Link);
    };
    let result = sqlx::query("update magic_links set pending_upload_id = $2 where token_hash = $1 and used_at is null and exists (select 1 from pending_uploads where id = $2 and expires_at > now())")
        .bind(hash_token(&raw).as_slice()).bind(upload_id).execute(pool).await?;
    Ok(result.rows_affected() == 1)
}

pub async fn find_session(
    pool: &sqlx::PgPool,
    session_id: Uuid,
    now: OffsetDateTime,
) -> Result<Option<Session>, AuthError> {
    store::find_session(pool, session_id, now).await
}

pub async fn list_sessions(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    now: OffsetDateTime,
) -> Result<Vec<ListedSession>, AuthError> {
    store::list_sessions(pool, user_id, now).await
}

pub async fn end_session(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<bool, AuthError> {
    store::end_session(pool, user_id, session_id).await
}

pub async fn delete_account(pool: &sqlx::PgPool, user_id: Uuid) -> Result<(), AuthError> {
    store::delete_account(pool, user_id).await
}

fn random_token() -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    let _ = getrandom::getrandom(&mut bytes);
    bytes
}

fn encode_token(token: [u8; 32]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, token)
}

fn decode_token(token: &str) -> Option<[u8; 32]> {
    if token.len() > 64 {
        return None;
    }
    let bytes =
        base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, token).ok()?;
    bytes.try_into().ok()
}

fn hash_token(token: &[u8; 32]) -> [u8; 32] {
    let digest = sha2::Sha256::digest(token);
    let mut hash = [0_u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

fn sign(secret: &str, session_id: Uuid) -> String {
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes())
        .expect("hmac accepts this key");
    mac.update(session_id.as_bytes());
    base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        mac.finalize().into_bytes(),
    )
}
