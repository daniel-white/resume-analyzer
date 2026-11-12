use auth::OffsetDateTime;
use sqlx::PgPool;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::ImageExt;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

struct Database {
    pool: PgPool,
    _container: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
}

async fn database() -> Database {
    let container = Postgres::default()
        .with_tag("17-alpine")
        .start()
        .await
        .expect("postgres");
    let host = container.get_host().await.expect("host");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect(&url)
        .await
        .expect("connect");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrate");
    Database {
        pool,
        _container: container,
    }
}

#[test]
fn magic_link_rejects_a_bad_email() {
    assert!(auth::normalize_email("not-an-email").is_none());
    assert!(auth::normalize_email("a@b").is_none());
    assert_eq!(
        auth::normalize_email(" Ada@Example.com ").as_deref(),
        Some("ada@example.com")
    );
}

#[test]
fn session_cookie_rejects_a_bad_signature() {
    let id = auth::Uuid::new_v4();
    let cookie = auth::seal_session("test-secret-value", id);
    assert_eq!(auth::open_session("test-secret-value", &cookie), Some(id));
    assert!(auth::open_session("other-secret-value", &cookie).is_none());
    let mut tampered = cookie.into_bytes();
    let last = tampered.len() - 1;
    tampered[last] = if tampered[last] == b'a' { b'b' } else { b'a' };
    let tampered = String::from_utf8(tampered).unwrap();
    assert!(auth::open_session("test-secret-value", &tampered).is_none());
}

#[tokio::test]
async fn magic_link_is_single_use_and_expires() {
    let database = database().await;
    let pool = &database.pool;
    let now = OffsetDateTime::now_utc();
    let issued = auth::request_magic_link(pool, "Ada@Example.com", now, None)
        .await
        .unwrap();
    assert_eq!(issued.email, "ada@example.com");

    let stored: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM magic_links WHERE email = $1")
        .bind(&issued.email)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(stored.len(), 32);
    assert_ne!(stored, issued.token.as_bytes());

    let session = auth::verify_magic_link(pool, &issued.token, now)
        .await
        .unwrap();
    assert_eq!(session.email, "ada@example.com");
    assert!(
        auth::verify_magic_link(pool, &issued.token, now)
            .await
            .is_err()
    );

    let again = auth::request_magic_link(pool, "ada@example.com", now, None)
        .await
        .unwrap();
    let later = now + time::Duration::minutes(16);
    assert!(
        auth::verify_magic_link(pool, &again.token, later)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn session_cookie_round_trip() {
    let database = database().await;
    let pool = &database.pool;
    let now = OffsetDateTime::now_utc();
    let first = auth::request_magic_link(pool, "ada@example.com", now, None)
        .await
        .unwrap();
    let session = auth::verify_magic_link(pool, &first.token, now)
        .await
        .unwrap();
    let cookie = auth::seal_session("test-secret-value", session.id);
    let opened = auth::open_session("test-secret-value", &cookie).unwrap();
    let found = auth::find_session(pool, opened, now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.email, "ada@example.com");

    let second = auth::request_magic_link(pool, "ada@example.com", now, None)
        .await
        .unwrap();
    let other = auth::verify_magic_link(pool, &second.token, now)
        .await
        .unwrap();
    assert_eq!(other.user_id, session.user_id);
    let listed = auth::list_sessions(pool, session.user_id, now)
        .await
        .unwrap();
    assert_eq!(listed.len(), 2);

    let stranger = auth::request_magic_link(pool, "grace@example.com", now, None)
        .await
        .unwrap();
    let grace = auth::verify_magic_link(pool, &stranger.token, now)
        .await
        .unwrap();
    assert!(
        !auth::end_session(pool, grace.user_id, session.id)
            .await
            .unwrap()
    );
    assert!(
        auth::find_session(pool, session.id, now)
            .await
            .unwrap()
            .is_some()
    );

    assert!(
        auth::end_session(pool, session.user_id, other.id)
            .await
            .unwrap()
    );
    assert!(
        auth::find_session(pool, other.id, now)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        auth::find_session(pool, session.id, now)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn delete_account_removes_sessions() {
    let database = database().await;
    let pool = &database.pool;
    let now = OffsetDateTime::now_utc();
    let issued = auth::request_magic_link(pool, "ada@example.com", now, None)
        .await
        .unwrap();
    let session = auth::verify_magic_link(pool, &issued.token, now)
        .await
        .unwrap();

    auth::delete_account(pool, session.user_id).await.unwrap();
    assert!(
        auth::find_session(pool, session.id, now)
            .await
            .unwrap()
            .is_none()
    );

    let again = auth::request_magic_link(pool, "ada@example.com", now, None)
        .await
        .unwrap();
    let fresh = auth::verify_magic_link(pool, &again.token, now)
        .await
        .unwrap();
    assert_ne!(fresh.user_id, session.user_id);
    assert!(
        auth::list_sessions(pool, fresh.user_id, now)
            .await
            .unwrap()
            .iter()
            .all(|row| row.id != session.id)
    );
}
