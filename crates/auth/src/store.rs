use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{AuthError, hash_token};

pub struct IssuedLink {
    pub token: String,
    pub email: String,
    pub expires_at: OffsetDateTime,
}

pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
}

pub struct ListedSession {
    pub id: Uuid,
    pub created_at: OffsetDateTime,
}

pub async fn insert_magic_link(
    pool: &PgPool,
    email: &str,
    token: &[u8; 32],
    now: OffsetDateTime,
    expires_at: OffsetDateTime,
) -> Result<(), AuthError> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE magic_links SET used_at = $2 WHERE email = $1 AND used_at IS NULL")
        .bind(email)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO magic_links (id, email, token_hash, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(hash_token(token).as_slice())
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn consume_magic_link(
    pool: &PgPool,
    token: &[u8; 32],
    now: OffsetDateTime,
    session_expires_at: OffsetDateTime,
) -> Result<Session, AuthError> {
    let mut tx = pool.begin().await?;
    let email: Option<String> = sqlx::query_scalar(
        "UPDATE magic_links
         SET used_at = $2
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > $2
         RETURNING email",
    )
    .bind(hash_token(token).as_slice())
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(email) = email else {
        return Err(AuthError::Link);
    };
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (id, email)
         VALUES ($1, $2)
         ON CONFLICT (email) DO UPDATE SET email = users.email
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(&email)
    .fetch_one(&mut *tx)
    .await?;
    let session_id = Uuid::new_v4();
    sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(session_id)
        .bind(user_id)
        .bind(session_expires_at)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Session {
        id: session_id,
        user_id,
        email,
    })
}

pub async fn find_session(
    pool: &PgPool,
    session_id: Uuid,
    now: OffsetDateTime,
) -> Result<Option<Session>, AuthError> {
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT sessions.id, users.id, users.email
         FROM sessions
         JOIN users ON users.id = sessions.user_id
         WHERE sessions.id = $1 AND sessions.expires_at > $2",
    )
    .bind(session_id)
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id, user_id, email)| Session { id, user_id, email }))
}

pub async fn list_sessions(
    pool: &PgPool,
    user_id: Uuid,
    now: OffsetDateTime,
) -> Result<Vec<ListedSession>, AuthError> {
    let rows: Vec<(Uuid, OffsetDateTime)> = sqlx::query_as(
        "SELECT id, created_at
         FROM sessions
         WHERE user_id = $1 AND expires_at > $2
         ORDER BY created_at",
    )
    .bind(user_id)
    .bind(now)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, created_at)| ListedSession { id, created_at })
        .collect())
}

pub async fn end_session(
    pool: &PgPool,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<bool, AuthError> {
    let result = sqlx::query("DELETE FROM sessions WHERE id = $1 AND user_id = $2")
        .bind(session_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_account(pool: &PgPool, user_id: Uuid) -> Result<(), AuthError> {
    let mut tx = pool.begin().await?;
    let email: Option<String> =
        sqlx::query_scalar("DELETE FROM users WHERE id = $1 RETURNING email")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(email) = email {
        sqlx::query("DELETE FROM magic_links WHERE email = $1")
            .bind(email)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
