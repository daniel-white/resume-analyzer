//! Upload-before-login stays private and is promoted exactly once.
use app::{Api, Bus, api_router, compiled_plugins};
use std::sync::Arc;
use storage::{MemoryObjects, Objects};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ImageExt, runners::AsyncRunner},
};

#[tokio::test]
async fn anonymous_upload_is_private_and_claimed_by_verified_link() {
    let container = Postgres::default()
        .with_tag("17-alpine")
        .start()
        .await
        .expect("postgres");
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let store = storage::Store::connect(&format!(
        "postgres://postgres:postgres@{host}:{port}/postgres"
    ))
    .await
    .unwrap();
    store.migrate().await.unwrap();
    let objects = Arc::new(MemoryObjects::default());
    let api = Arc::new(Api {
        store: store.clone(),
        objects: objects.clone(),
        plugins: compiled_plugins(),
        bus: Bus::default(),
        mail: mail::Smtp::new("127.0.0.1", 1),
        auth_secret: "test-secret-value".into(),
        app_origin: "http://127.0.0.1:3000".into(),
        cookie_secure: false,
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, api_router(api)).await.unwrap();
    });
    let client = reqwest::Client::new();
    let upload = client
        .post(format!("{base}/api/uploads"))
        .multipart(reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(b"%PDF-1.4\n%%EOF".to_vec()).file_name("chapter.pdf"),
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), reqwest::StatusCode::ACCEPTED);
    let id = upload.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse::<uuid::Uuid>()
        .unwrap();
    assert_eq!(
        client
            .get(format!("{base}/api/resumes/{id}"))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let before: i64 = sqlx::query_scalar("select count(*) from outbox")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(before, 0, "analysis waits until the email is verified");
    let issued = auth::request_magic_link(
        store.pool(),
        "ada@example.com",
        auth::OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    assert!(
        auth::bind_upload(store.pool(), &issued.token, id)
            .await
            .unwrap()
    );
    let verified = client
        .post(format!("{base}/api/auth/magic-link/verify"))
        .json(&serde_json::json!({ "token": issued.token }))
        .send()
        .await
        .unwrap();
    assert_eq!(verified.status(), reqwest::StatusCode::OK);
    let cookie = verified.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let detail = client
        .get(format!("{base}/api/resumes/{id}"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(detail.status(), reqwest::StatusCode::OK);
    assert_eq!(
        detail.json::<serde_json::Value>().await.unwrap()["versions"][0]["filename"],
        "chapter.pdf"
    );
    let event: String = sqlx::query_scalar("select name from outbox")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(event, "ResumeUploaded");
    assert!(
        !auth::bind_upload(store.pool(), &issued.token, id)
            .await
            .unwrap()
    );
    assert!(
        objects.get(&format!("pending/{id}")).await.is_ok(),
        "claimed bytes are retained"
    );

    let expired = uuid::Uuid::new_v4();
    let key = format!("pending/{expired}");
    objects.put(&key, b"%PDF").await.unwrap();
    store
        .stage_upload(expired, "expired.pdf", domain::PDF, &key)
        .await
        .unwrap();
    sqlx::query(
        "update pending_uploads set expires_at = now() - interval '1 second' where id = $1",
    )
    .bind(expired)
    .execute(store.pool())
    .await
    .unwrap();
    let other = auth::request_magic_link(
        store.pool(),
        "other@example.com",
        auth::OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    assert!(
        !auth::bind_upload(store.pool(), &other.token, expired)
            .await
            .unwrap()
    );
    store.clean_pending_uploads(objects.as_ref()).await.unwrap();
    assert!(objects.get(&key).await.is_err());
    assert!(objects.get(&format!("pending/{id}")).await.is_ok());

    // A link bound before expiry cannot revive expired bytes; the whole
    // verification transaction rolls back, including user/session creation.
    let stale = uuid::Uuid::new_v4();
    store
        .stage_upload(stale, "stale.pdf", domain::PDF, "pending/stale")
        .await
        .unwrap();
    let stale_link = auth::request_magic_link(
        store.pool(),
        "stale@example.com",
        auth::OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    assert!(
        auth::bind_upload(store.pool(), &stale_link.token, stale)
            .await
            .unwrap()
    );
    sqlx::query(
        "update pending_uploads set expires_at = now() - interval '1 second' where id = $1",
    )
    .bind(stale)
    .execute(store.pool())
    .await
    .unwrap();
    assert!(matches!(
        auth::verify_magic_link(
            store.pool(),
            &stale_link.token,
            auth::OffsetDateTime::now_utc()
        )
        .await,
        Err(auth::AuthError::Link)
    ));
    let users: i64 =
        sqlx::query_scalar("select count(*) from users where email = 'stale@example.com'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(users, 0);

    // Knowing a resume id never authorizes a different account to read it.
    let other_session =
        auth::verify_magic_link(store.pool(), &other.token, auth::OffsetDateTime::now_utc())
            .await
            .unwrap();
    assert!(
        store
            .detail(other_session.user_id, id)
            .await
            .unwrap()
            .is_none()
    );

    let unsupported = client
        .post(format!("{base}/api/uploads"))
        .multipart(reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(b"not a PDF".to_vec()).file_name("fake.pdf"),
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        unsupported.status(),
        reqwest::StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}
