//! One upload, through the router, reaches one stub callback on the live stream.

use std::io::{Cursor, Write};
use std::sync::Arc;

use app::{Api, Bus, Worker, api_router, compiled_plugins};
use domain::{DOCX, PDF};
use futures_util::StreamExt;
use storage::MemoryObjects;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::ImageExt;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

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

fn designed_docx() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer.start_file("word/header1.xml", stored).unwrap();
    writer
        .write_all(b"<w:t>Ada Lovelace</w:t><w:t>ada@example.com</w:t>")
        .unwrap();
    writer.start_file("word/document.xml", stored).unwrap();
    writer
        .write_all(
            br#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Experience</w:t></w:r></w:p><w:tbl></w:tbl>"#,
        )
        .unwrap();
    writer.finish().unwrap().into_inner()
}

fn plain_pdf() -> Vec<u8> {
    b"%PDF-1.4\n(Ada Lovelace) Tj (ada@example.com) Tj\n%%EOF\n".to_vec()
}

fn multipart(filename: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    let boundary = "spine-boundary";
    let mut body = Vec::new();
    body.extend(format!("--{boundary}\r\n").into_bytes());
    body.extend(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .into_bytes(),
    );
    body.extend(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend(bytes);
    body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn upload(base: &str, cookie: &str, filename: &str, bytes: &[u8]) -> serde_json::Value {
    let (content_type, body) = multipart(filename, bytes);
    let response = reqwest::Client::new()
        .post(format!("{base}/api/resumes"))
        .header("cookie", cookie)
        .header("content-type", content_type)
        .body(body)
        .send()
        .await
        .expect("upload")
        .error_for_status()
        .expect("accepted");
    response.json().await.expect("accepted json")
}

/// Subscribe before any outbox row is claimed. The stream does not replay, so
/// the worker stays idle until the response headers are back.
async fn callback_on_the_stream(
    base: &str,
    cookie: &str,
    resume_id: &str,
    worker: &Worker,
) -> Vec<serde_json::Value> {
    let response = reqwest::Client::new()
        .get(format!("{base}/api/resumes/{resume_id}/events"))
        .header("cookie", cookie)
        .send()
        .await
        .expect("stream");
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    worker.drain().await.expect("drain");
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut found = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        found.extend(take_callbacks(&mut buffer));
        let quiet = if found.is_empty() {
            std::time::Duration::from_secs(2)
        } else {
            // The callback is last. A short quiet period catches a second frame
            // that has not been written yet.
            std::time::Duration::from_millis(200)
        };
        if tokio::time::Instant::now() > deadline {
            break;
        }
        let chunk = tokio::time::timeout(quiet, stream.next()).await;
        let Ok(chunk) = chunk else {
            break;
        };
        let Some(chunk) = chunk else {
            break;
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk.expect("chunk")));
    }
    if found.is_empty() {
        panic!("no callback on the stream: {buffer}");
    }
    found
}

fn take_callbacks(buffer: &mut String) -> Vec<serde_json::Value> {
    let mut found = Vec::new();
    while let Some(split) = buffer.find("\n\n") {
        let frame = buffer[..split].to_owned();
        buffer.replace_range(..split + 2, "");
        let mut name = String::new();
        let mut data = String::new();
        for line in frame.lines() {
            if let Some(value) = line.strip_prefix("event:") {
                name = value.trim().to_owned();
            } else if let Some(value) = line.strip_prefix("data:") {
                data.push_str(value.trim());
            }
        }
        if name == "AtsCallbackReceived" {
            found.push(serde_json::from_str(&data).expect("event json"));
        }
    }
    found
}

async fn one_callback(
    base: &str,
    cookie: &str,
    worker: &Worker,
    filename: &str,
    media_type: &str,
    bytes: &[u8],
    score: u8,
) {
    let accepted = upload(base, cookie, filename, bytes).await;
    assert_eq!(accepted["status"], "uploaded");
    let resume_id = accepted["id"].as_str().expect("resume id");
    let version_id = accepted["version_id"].as_str().expect("version id");

    let callbacks = callback_on_the_stream(base, cookie, resume_id, worker).await;
    assert_eq!(callbacks.len(), 1, "one stub callback on the stream");
    let event = &callbacks[0];
    assert_eq!(event["name"], "AtsCallbackReceived");
    assert_eq!(event["resume_id"], resume_id);
    assert_eq!(event["version_id"], version_id);
    assert_eq!(event["payload"]["plugin_id"], "stub");
    assert_eq!(event["payload"]["verdict"]["score"], score);
    let summary = event["payload"]["verdict"]["summary"]
        .as_str()
        .expect("summary");
    assert!(summary.starts_with(&format!("Score {score} of 100.")));

    let detail: serde_json::Value = reqwest::Client::new()
        .get(format!("{base}/api/resumes/{resume_id}"))
        .header("cookie", cookie)
        .send()
        .await
        .expect("detail")
        .json()
        .await
        .expect("detail json");
    let version = &detail["versions"][0];
    assert_eq!(version["id"], version_id);
    assert_eq!(version["media_type"], media_type);
    assert_eq!(version["status"], "complete");
    assert!(version["parse"].is_object());
    assert_eq!(version["runs"].as_array().expect("runs").len(), 1);
    assert_eq!(version["runs"][0]["plugin_id"], "stub");
    assert_eq!(version["runs"][0]["status"], "complete");
    assert!(version["runs"][0]["parse"].is_object());
    assert_eq!(version["runs"][0]["verdict"]["score"], score);
    assert_eq!(version["runs"][0]["verdict"]["summary"], summary);
    // These are the fields consumed by the restored frontend. The app's
    // verification extract and each provider's own extract stay independent.
    assert!(version["parse"]["profile"]["sections"].is_array());
    assert!(version["parse"]["layout"]["columns"].is_number());
    let verdict = &version["runs"][0]["verdict"];
    assert!(verdict["kept"].is_array());
    assert!(verdict["dropped"].is_array());
    let factors = verdict["factors"].as_array().expect("frontend factors");
    assert!(!factors.is_empty());
    for factor in factors {
        assert!(factor["label"].is_string());
        assert!(factor["delta"].is_i64());
        assert!(!factor["advice"].as_str().expect("factor advice").is_empty());
    }
}

#[tokio::test]
async fn upload_reaches_a_callback() {
    let database = database().await;
    let store = storage::Store::connect(&database.url)
        .await
        .expect("connect");
    store.migrate().await.expect("migrate");
    let objects = Arc::new(MemoryObjects::default());
    let plugins = compiled_plugins();
    let bus = Bus::default();
    let secret = "test-secret-value";
    let now = auth::OffsetDateTime::now_utc();
    let issued = auth::request_magic_link(store.pool(), "ada@example.com", now)
        .await
        .expect("link");
    let session = auth::verify_magic_link(store.pool(), &issued.token, now)
        .await
        .expect("session");
    let cookie = format!("session={}", auth::seal_session(secret, session.id));
    let api = Arc::new(Api {
        store: store.clone(),
        objects: objects.clone(),
        plugins: plugins.clone(),
        bus: bus.clone(),
        mail: mail::Smtp::new("127.0.0.1", 1),
        auth_secret: secret.to_owned(),
        app_origin: "http://127.0.0.1:3000".to_owned(),
        cookie_secure: false,
    });
    let app = api_router(api);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("address");
    let base = format!("http://{address}");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    let worker = Worker::new(store, objects, plugins, bus, base.clone());

    one_callback(
        &base,
        &cookie,
        &worker,
        "designed.docx",
        DOCX,
        &designed_docx(),
        15,
    )
    .await;
    one_callback(&base, &cookie, &worker, "plain.pdf", PDF, &plain_pdf(), 65).await;

    let listed: serde_json::Value = reqwest::Client::new()
        .get(format!("{base}/api/resumes"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("list")
        .json()
        .await
        .expect("list json");
    assert_eq!(listed["resumes"].as_array().expect("resumes").len(), 2);

    let plugins: serde_json::Value = reqwest::get(format!("{base}/api/plugins"))
        .await
        .expect("plugins")
        .json()
        .await
        .expect("plugins json");
    assert_eq!(plugins["plugins"][0]["id"], "stub");
}
