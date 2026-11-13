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
    /// Browser-reported IANA timezone, carried from the magic link that
    /// created the session. Absent when the client did not send one.
    pub timezone: Option<String>,
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
    timezone: Option<&str>,
) -> Result<(), AuthError> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE magic_links SET used_at = $2 WHERE email = $1 AND used_at IS NULL")
        .bind(email)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO magic_links (id, email, token_hash, expires_at, timezone) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(hash_token(token).as_slice())
    .bind(expires_at)
    .bind(timezone)
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
    let link: Option<(String, Option<Uuid>, Option<String>)> = sqlx::query_as(
        "UPDATE magic_links
         SET used_at = $2
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > $2
         RETURNING email, pending_upload_id, timezone",
    )
    .bind(hash_token(token).as_slice())
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((email, upload_id, timezone)) = link else {
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
    if let Some(id) = upload_id {
        let pending: Option<(String, String, String)> = sqlx::query_as(
            "delete from pending_uploads where id = $1 and expires_at > $2 returning filename, media_type, object_key"
        ).bind(id).bind(now).fetch_optional(&mut *tx).await?;
        let Some((filename, media_type, object_key)) = pending else {
            return Err(AuthError::Link);
        };
        let version_id = Uuid::new_v4();
        sqlx::query("insert into resumes (id, user_id, title) values ($1, $2, $3)")
            .bind(id)
            .bind(user_id)
            .bind(&filename)
            .execute(&mut *tx)
            .await?;
        sqlx::query("insert into versions (id, resume_id, number, filename, media_type, object_key, status) values ($1, $2, 1, $3, $4, $5, 'uploaded')")
            .bind(version_id).bind(id).bind(&filename).bind(&media_type).bind(&object_key).execute(&mut *tx).await?;
        sqlx::query("insert into outbox (id, resume_id, version_id, name, payload) values ($1, $2, $3, 'ResumeUploaded', jsonb_build_object('user_id', $4::text, 'resume_id', $2::text, 'version_id', $3::text, 'filename', $5::text, 'media_type', $6::text, 'object_key', $7::text))")
            .bind(Uuid::new_v4()).bind(id).bind(version_id).bind(user_id.to_string()).bind(filename).bind(media_type).bind(object_key).execute(&mut *tx).await?;
    }
    let session_id = Uuid::new_v4();
    sqlx::query("INSERT INTO sessions (id, user_id, expires_at, timezone) VALUES ($1, $2, $3, $4)")
        .bind(session_id)
        .bind(user_id)
        .bind(session_expires_at)
        .bind(&timezone)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Session {
        id: session_id,
        user_id,
        email,
        timezone,
    })
}

pub async fn find_session(
    pool: &PgPool,
    session_id: Uuid,
    now: OffsetDateTime,
) -> Result<Option<Session>, AuthError> {
    let row: Option<(Uuid, Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT sessions.id, users.id, users.email, sessions.timezone
         FROM sessions
         JOIN users ON users.id = sessions.user_id
         WHERE sessions.id = $1 AND sessions.expires_at > $2",
    )
    .bind(session_id)
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id, user_id, email, timezone)| Session {
        id,
        user_id,
        email,
        timezone,
    }))
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
