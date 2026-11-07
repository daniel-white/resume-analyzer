//! Magic-link sign-in, and deleting the account takes the resumes with it.

use std::sync::Arc;

use app::{Api, Bus, api_router, compiled_plugins};
use storage::{MemoryObjects, Objects};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::ImageExt;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

struct Database {
    url: String,
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
    Database {
        url: format!("postgres://postgres:postgres@{host}:{port}/postgres"),
        _container: container,
    }
}

fn plain_pdf() -> Vec<u8> {
    b"%PDF-1.4\n(Ada Lovelace) Tj (ada@example.com) Tj\n%%EOF\n".to_vec()
}

fn multipart(bytes: &[u8]) -> (String, Vec<u8>) {
    let boundary = "account-boundary";
    let mut body = Vec::new();
    body.extend(format!("--{boundary}\r\n").into_bytes());
    body.extend(
        "Content-Disposition: form-data; name=\"file\"; filename=\"ada.pdf\"\r\n".as_bytes(),
    );
    body.extend(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend(bytes);
    body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

fn session_pair(response: &reqwest::Response) -> String {
    let header = response
        .headers()
        .get("set-cookie")
        .expect("set-cookie")
        .to_str()
        .expect("cookie ascii");
    assert!(header.contains("HttpOnly"));
    assert!(header.contains("SameSite=Lax"));
    header.split(';').next().expect("cookie pair").to_owned()
}

async fn sign_in(base: &str, store: &storage::Store, email: &str) -> (String, String) {
    let now = auth::OffsetDateTime::now_utc();
    let issued = auth::request_magic_link(store.pool(), email, now)
        .await
        .expect("link");
    let response = reqwest::Client::new()
        .post(format!("{base}/api/auth/magic-link/verify"))
        .json(&serde_json::json!({ "token": issued.token }))
        .send()
        .await
        .expect("verify");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let cookie = session_pair(&response);
    let body: serde_json::Value = response.json().await.expect("identity");
    let user_id = body["id"].as_str().expect("user id").to_owned();
    assert_eq!(body["email"], email);
    (cookie, user_id)
}

#[tokio::test]
async fn login_cookie_can_manage_the_account() {
    let database = database().await;
    let store = storage::Store::connect(&database.url)
        .await
        .expect("connect");
    store.migrate().await.expect("migrate");
    let objects = Arc::new(MemoryObjects::default());
    let api = Arc::new(Api {
        store: store.clone(),
        objects: objects.clone(),
        plugins: compiled_plugins(),
        bus: Bus::default(),
        mail: mail::Smtp::new("127.0.0.1", 1),
        auth_secret: "test-secret-value".to_owned(),
        app_origin: "http://127.0.0.1:3000".to_owned(),
        cookie_secure: false,
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("address");
    let base = format!("http://{address}");
    tokio::spawn(async move {
        axum::serve(listener, api_router(api)).await.expect("serve");
    });
    let client = reqwest::Client::new();

    let missing = client
        .get(format!("{base}/api/me"))
        .send()
        .await
        .expect("me");
    assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);

    let bad_email = client
        .post(format!("{base}/api/auth/magic-link"))
        .json(&serde_json::json!({ "email": "not-an-email" }))
        .send()
        .await
        .expect("magic link");
    assert_eq!(bad_email.status(), reqwest::StatusCode::BAD_REQUEST);

    let bad_token = client
        .post(format!("{base}/api/auth/magic-link/verify"))
        .json(&serde_json::json!({ "token": "nope" }))
        .send()
        .await
        .expect("verify");
    assert_eq!(bad_token.status(), reqwest::StatusCode::BAD_REQUEST);

    let unmailed = client
        .post(format!("{base}/api/auth/magic-link"))
        .json(&serde_json::json!({ "email": "ada@example.com" }))
        .send()
        .await
        .expect("mail");
    assert_eq!(unmailed.status(), reqwest::StatusCode::BAD_GATEWAY);

    let logout = client
        .post(format!("{base}/api/auth/logout"))
        .send()
        .await
        .expect("logout");
    assert_eq!(logout.status(), reqwest::StatusCode::NO_CONTENT);
    let cleared = logout
        .headers()
        .get("set-cookie")
        .expect("clear")
        .to_str()
        .expect("ascii");
    assert!(cleared.contains("Max-Age=0"));
    assert!(cleared.contains("HttpOnly"));

    let (ada_cookie, ada_id) = sign_in(&base, &store, "ada@example.com").await;
    let (grace_cookie, _) = sign_in(&base, &store, "grace@example.com").await;

    let me: serde_json::Value = client
        .get(format!("{base}/api/me"))
        .header("cookie", &ada_cookie)
        .send()
        .await
        .expect("me")
        .json()
        .await
        .expect("me json");
    assert_eq!(me["email"], "ada@example.com");
    assert_eq!(me["sessions"][0]["current"], true);

    let (content_type, body) = multipart(&plain_pdf());
    let uploaded: serde_json::Value = client
        .post(format!("{base}/api/resumes"))
        .header("cookie", &ada_cookie)
        .header("content-type", content_type)
        .body(body)
        .send()
        .await
        .expect("upload")
        .error_for_status()
        .expect("accepted")
        .json()
        .await
        .expect("accepted json");
    let resume_id = uploaded["id"].as_str().expect("resume");
    let version_id = uploaded["version_id"].as_str().expect("version");
    let ada_uuid = uuid::Uuid::parse_str(&ada_id).expect("ada uuid");
    let object_key = domain::version_object_key(
        ada_uuid,
        uuid::Uuid::parse_str(resume_id).expect("resume uuid"),
        uuid::Uuid::parse_str(version_id).expect("version uuid"),
    );
    objects.get(&object_key).await.expect("stored object");

    let hidden = client
        .get(format!("{base}/api/resumes/{resume_id}"))
        .header("cookie", &grace_cookie)
        .send()
        .await
        .expect("hidden");
    assert_eq!(hidden.status(), reqwest::StatusCode::NOT_FOUND);

    let session_id = me["sessions"][0]["id"].as_str().expect("session");
    let refused = client
        .delete(format!("{base}/api/auth/sessions/{session_id}"))
        .header("cookie", &grace_cookie)
        .send()
        .await
        .expect("end");
    assert_eq!(refused.status(), reqwest::StatusCode::NOT_FOUND);

    let removed = client
        .delete(format!("{base}/api/me"))
        .header("cookie", &ada_cookie)
        .send()
        .await
        .expect("delete");
    assert_eq!(removed.status(), reqwest::StatusCode::NO_CONTENT);
    assert!(objects.get(&object_key).await.is_err());
    assert!(
        store
            .detail(
                ada_uuid,
                uuid::Uuid::parse_str(resume_id).expect("resume uuid")
            )
            .await
            .expect("detail")
            .is_none()
    );

    let gone = client
        .get(format!("{base}/api/me"))
        .header("cookie", &ada_cookie)
        .send()
        .await
        .expect("gone");
    assert_eq!(gone.status(), reqwest::StatusCode::UNAUTHORIZED);

    let grace_list: serde_json::Value = client
        .get(format!("{base}/api/resumes"))
        .header("cookie", &grace_cookie)
        .send()
        .await
        .expect("grace list")
        .json()
        .await
        .expect("grace json");
    assert_eq!(grace_list["resumes"].as_array().expect("resumes").len(), 0);

    let again = sign_in(&base, &store, "ada@example.com").await;
    assert_ne!(again.1, ada_id);
}
