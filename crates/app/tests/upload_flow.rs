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

/// Small SMTP sink verifies actual delivery without another external service.
async fn smtp_sink() -> (u16, tokio::sync::mpsc::Receiver<String>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = tokio::sync::mpsc::channel(8);
    tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let sender = sender.clone();
            tokio::spawn(async move {
                let (read, mut write) = socket.into_split();
                let mut lines = BufReader::new(read).lines();
                write.write_all(b"220 localhost SMTP\r\n").await.unwrap();
                let mut data = false;
                let mut body = String::new();
                while let Some(line) = lines.next_line().await.unwrap() {
                    if data {
                        if line != "." {
                            body.push_str(&line);
                            body.push('\n');
                            continue;
                        }
                        sender.send(std::mem::take(&mut body)).await.unwrap();
                        data = false;
                        write.write_all(b"250 accepted\r\n").await.unwrap();
                    } else if line.starts_with("EHLO") {
                        write
                            .write_all(b"250-localhost\r\n250 8BITMIME\r\n")
                            .await
                            .unwrap();
                    } else if line == "DATA" {
                        data = true;
                        write.write_all(b"354 send data\r\n").await.unwrap();
                    } else if line == "QUIT" {
                        write.write_all(b"221 bye\r\n").await.unwrap();
                        break;
                    } else {
                        write.write_all(b"250 ok\r\n").await.unwrap();
                    }
                }
            });
        }
    });
    (port, receiver)
}

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
    let boundary = "upload-boundary";
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

#[tokio::test]
async fn blast_track_reconciles_a_completed_resume_and_completes_new_uploads() {
    let database = database().await;
    let store = storage::Store::connect(&database.url).await.unwrap();
    store.migrate().await.unwrap();
    let objects = Arc::new(MemoryObjects::default());
    let auth_secret = "blast-track-auth-test";
    let now = auth::OffsetDateTime::now_utc();
    let issued = auth::request_magic_link(store.pool(), "blast@example.com", now, None)
        .await
        .unwrap();
    let session = auth::verify_magic_link(store.pool(), &issued.token, now)
        .await
        .unwrap();
    let cookie = format!("session={}", auth::seal_session(auth_secret, session.id));
    let old_plugins: Vec<Arc<dyn domain::Ats>> =
        vec![Arc::new(plugins::Stub::new("test-plugin-secret"))];
    let mut plugins = old_plugins.clone();
    plugins.push(Arc::new(plugins::BlastTrack::new("test-plugin-secret")));
    let bus = Bus::default();
    let api = Arc::new(Api {
        store: store.clone(),
        objects: objects.clone(),
        plugins: plugins.clone(),
        bus: bus.clone(),
        mail: mail::Smtp::new("127.0.0.1", 1),
        auth_secret: auth_secret.into(),
        app_origin: "http://127.0.0.1:3000".into(),
        cookie_secure: false,
        admin_email_domains: vec![],
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, api_router(api)).await.unwrap() });
    let old_worker = Worker::new(
        store.clone(),
        objects.clone(),
        old_plugins,
        bus.clone(),
        base.clone(),
    );
    let worker = Worker::new(store.clone(), objects, plugins, bus, base.clone());
    let client = reqwest::Client::new();
    let accepted = upload(&base, &cookie, "before.pdf", &plain_pdf()).await;
    let id = accepted["id"].as_str().unwrap();
    let version = uuid::Uuid::parse_str(accepted["version_id"].as_str().unwrap()).unwrap();
    old_worker.drain().await.unwrap();
    let old_verdict: serde_json::Value =
        sqlx::query_scalar("select verdict from runs where version_id = $1 and plugin_id = 'stub'")
            .bind(version)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let status: String = sqlx::query_scalar("select status from versions where id = $1")
        .bind(version)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(status, "complete");
    let url = format!("{base}/api/resumes/{id}");
    assert_eq!(
        client.get(&url).send().await.unwrap().status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let other = auth::request_magic_link(store.pool(), "other@example.com", now, None)
        .await
        .unwrap();
    let other_session = auth::verify_magic_link(store.pool(), &other.token, now)
        .await
        .unwrap();
    let other_cookie = format!(
        "session={}",
        auth::seal_session(auth_secret, other_session.id)
    );
    assert_eq!(
        client
            .get(&url)
            .header("cookie", other_cookie)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    let count: i64 = sqlx::query_scalar("select count(*) from runs where version_id = $1")
        .bind(version)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let (left, right) = tokio::join!(
        client.get(&url).header("cookie", &cookie).send(),
        client.get(&url).header("cookie", &cookie).send(),
    );
    for response in [left.unwrap(), right.unwrap()] {
        response.error_for_status().unwrap();
    }
    // Concurrent reads can straddle the transaction. Check the authoritative
    // state after both visits finish, then verify another visit creates no work.
    let reconciled: serde_json::Value = client
        .get(&url)
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(reconciled["versions"][0]["status"], "running");
    assert_eq!(
        reconciled["versions"][0]["runs"].as_array().unwrap().len(),
        2
    );
    let requested: i64 = sqlx::query_scalar("select count(*) from outbox where version_id = $1 and name = 'AtsRunRequested' and payload->>'plugin_id' = 'blast_track'")
        .bind(version).fetch_one(store.pool()).await.unwrap();
    assert_eq!(requested, 1);
    worker.drain().await.unwrap();
    let detail: serde_json::Value = client
        .get(&url)
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["versions"][0]["status"], "complete");
    let runs = detail["versions"][0]["runs"].as_array().unwrap();
    assert_eq!(
        runs.iter().find(|run| run["plugin_id"] == "stub").unwrap()["verdict"],
        old_verdict
    );
    assert_eq!(
        runs.iter()
            .find(|run| run["plugin_id"] == "blast_track")
            .unwrap()["status"],
        "complete"
    );
    let fresh = upload(&base, &cookie, "new.pdf", &plain_pdf()).await;
    worker.drain().await.unwrap();
    let fresh: serde_json::Value = client
        .get(format!(
            "{base}/api/resumes/{}",
            fresh["id"].as_str().unwrap()
        ))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(fresh["versions"][0]["status"], "complete");
    assert_eq!(fresh["versions"][0]["runs"].as_array().unwrap().len(), 2);
    server.abort();
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
    assert!(version["profile_summary"]["text"].is_string());
    assert_eq!(version["profile_summary"]["engine"], "extractive");
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
    let issued = auth::request_magic_link(store.pool(), "ada@example.com", now, None)
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
        admin_email_domains: vec![],
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
    let (smtp_port, mut mail) = smtp_sink().await;
    let worker = Worker::new(store.clone(), objects, plugins, bus, base.clone()).notifications(
        mail::Smtp::new("127.0.0.1", smtp_port),
        "http://127.0.0.1:3000",
    );

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

    assert!(
        !worker.send_analysis_batch().await.unwrap(),
        "batch window hasn't elapsed"
    );
    sqlx::query("update runs set notification_due_at = now() - interval '1 second'")
        .execute(store.pool())
        .await
        .unwrap();
    for _ in 0..2 {
        assert!(worker.send_analysis_batch().await.unwrap());
        let message = tokio::time::timeout(std::time::Duration::from_secs(5), mail.recv())
            .await
            .unwrap()
            .unwrap();
        // Long deep links use MIME quoted-printable: unfold its soft line breaks
        // and encoded equals signs before asserting the user-visible URL.
        // The accented word in the subject is an RFC 2047 base64 encoded word.
        assert!(
            message.contains("Subject: Your =?utf-8?b?csOpc3Vtw6k=?= analysis results are ready")
        );
        let message = message.replace("=\n", "").replace("=3D", "=");
        assert!(message.contains("To: ada@example.com"));
        assert!(message.contains("http://127.0.0.1:3000/resumes/"));
        assert!(message.contains("?version="));
        assert!(message.contains("&provider=stub"));
    }
    assert!(
        !worker.send_analysis_batch().await.unwrap(),
        "sent results don't repeat"
    );

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

/// A real Python HTTP handoff after registry growth preserves completed runs.
#[tokio::test]
async fn indy_hackers_hiring_reconciles_old_resumes_and_completes_new_uploads() {
    use domain::{Ats, ResumeDetail, ResumeStatus};
    use std::collections::HashMap;
    use std::process::{Child, Command, Stdio};

    struct ProviderProcess(Child);
    impl Drop for ProviderProcess {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let database = database().await;
    let store = storage::Store::connect(&database.url).await.unwrap();
    store.migrate().await.unwrap();
    let objects = Arc::new(MemoryObjects::default());
    let now = auth::OffsetDateTime::now_utc();
    let issued = auth::request_magic_link(store.pool(), "indy-test@example.com", now, None)
        .await
        .unwrap();
    let session = auth::verify_magic_link(store.pool(), &issued.token, now)
        .await
        .unwrap();
    let auth_secret = "indy-auth-test-secret";
    let cookie = format!("session={}", auth::seal_session(auth_secret, session.id));
    let plugin_secret = "indy-callback-test-secret";
    let original_plugins: Vec<Arc<dyn Ats>> = vec![Arc::new(plugins::Stub::new(plugin_secret))];
    let bus = Bus::default();
    let serve = |plugins: Vec<Arc<dyn Ats>>| {
        let api = Arc::new(Api {
            store: store.clone(),
            objects: objects.clone(),
            plugins,
            bus: bus.clone(),
            mail: mail::Smtp::new("127.0.0.1", 1),
            auth_secret: auth_secret.into(),
            app_origin: "http://127.0.0.1:3000".into(),
            cookie_secure: false,
            admin_email_domains: vec![],
        });
        async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let task = tokio::spawn(async move {
                axum::serve(listener, api_router(api)).await.unwrap();
            });
            (base, task)
        }
    };
    let (old_base, old_server) = serve(original_plugins.clone()).await;
    let old_worker = Worker::new(
        store.clone(),
        objects.clone(),
        original_plugins.clone(),
        bus.clone(),
        old_base.clone(),
    );
    let old = upload(&old_base, &cookie, "old.pdf", &plain_pdf()).await;
    old_worker.drain().await.unwrap();
    let client = reqwest::Client::new();
    let old_detail: ResumeDetail = client
        .get(format!(
            "{old_base}/api/resumes/{}",
            old["id"].as_str().unwrap()
        ))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(old_detail.versions[0].status, ResumeStatus::Complete);
    let original_run = old_detail.versions[0].runs[0].clone();

    // Leave the previous completed version untouched when reconciling the newest.
    let second = client
        .post(format!("{old_base}/api/resumes/{}/versions", old_detail.id))
        .header("cookie", &cookie)
        .header("content-type", multipart("new.pdf", &plain_pdf()).0)
        .body(multipart("new.pdf", &plain_pdf()).1)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), reqwest::StatusCode::ACCEPTED);
    old_worker.drain().await.unwrap();
    let completed: ResumeDetail = client
        .get(format!("{old_base}/api/resumes/{}", old_detail.id))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(completed.versions[0].status, ResumeStatus::Complete);
    let newest_original_run = completed.versions[0].runs[0].clone();

    let mut active = original_plugins;
    active.push(Arc::new(plugins::IndyHackersHiring::new(plugin_secret)));
    let (base, server) = serve(active.clone()).await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let python = std::env::var("PYTHON3").unwrap_or_else(|_| "python3".into());
    let mut command = Command::new(python);
    #[cfg(target_os = "macos")]
    command.env_remove("DEVELOPER_DIR");
    let mut provider = ProviderProcess(
        command
            .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .args([
                "-m",
                "providers",
                "indy_hackers_hiring",
                "--port",
                &port.to_string(),
            ])
            .env("PLUGIN_SECRET", plugin_secret)
            .env("ATS_DELAY", "0,0")
            .stdout(Stdio::null())
            .spawn()
            .expect("Python provider"),
    );
    let endpoint = format!("http://127.0.0.1:{port}");
    let mut healthy = false;
    for _ in 0..100 {
        if let Ok(response) = client.get(format!("{endpoint}/health")).send().await {
            let body: serde_json::Value = response.json().await.unwrap();
            assert_eq!(body["id"], "indy_hackers_hiring");
            healthy = true;
            break;
        }
        assert!(
            provider.0.try_wait().unwrap().is_none(),
            "provider exited before health"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(healthy, "provider never became healthy");
    let worker = Worker::new(store.clone(), objects, active, bus, base.clone())
        .endpoints(HashMap::from([("indy_hackers_hiring".into(), endpoint)]));
    let detail_url = format!("{base}/api/resumes/{}", old_detail.id);
    assert_eq!(
        client.get(&detail_url).send().await.unwrap().status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let other_link = auth::request_magic_link(store.pool(), "other-indy@example.com", now, None)
        .await
        .unwrap();
    let other = auth::verify_magic_link(store.pool(), &other_link.token, now)
        .await
        .unwrap();
    let other_cookie = format!("session={}", auth::seal_session(auth_secret, other.id));
    assert_eq!(
        client
            .get(&detail_url)
            .header("cookie", other_cookie)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    let count: i64 =
        sqlx::query_scalar("select count(*) from runs where plugin_id = 'indy_hackers_hiring'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(count, 0);
    let (left, right) = tokio::join!(
        client.get(&detail_url).header("cookie", &cookie).send(),
        client.get(&detail_url).header("cookie", &cookie).send(),
    );
    for response in [left.unwrap(), right.unwrap()] {
        let detail: ResumeDetail = response.json().await.unwrap();
        assert_eq!(detail.versions[0].runs.len(), 2);
        assert!(detail.versions[0].runs.contains(&newest_original_run));
        assert_eq!(detail.versions[1], old_detail.versions[0]);
        assert_eq!(detail.versions[1].runs[0], original_run);
    }
    let repeated: ResumeDetail = client
        .get(&detail_url)
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // A concurrent response may read the version status before the other
    // request commits its fanout. Once both finish, authoritative state is running.
    assert_eq!(repeated.versions[0].status, ResumeStatus::Running);
    assert_eq!(repeated.versions[0].runs.len(), 2);
    let count: i64 = sqlx::query_scalar("select count(*) from outbox where name = 'AtsRunRequested' and payload->>'plugin_id' = 'indy_hackers_hiring'")
        .fetch_one(store.pool()).await.unwrap();
    assert_eq!(count, 1);

    let fresh = upload(&base, &cookie, "fresh.docx", &designed_docx()).await;
    worker.drain().await.unwrap();
    for resume_id in [
        old_detail.id.to_string(),
        fresh["id"].as_str().unwrap().to_owned(),
    ] {
        let mut complete = false;
        for _ in 0..100 {
            let detail: ResumeDetail = client
                .get(format!("{base}/api/resumes/{resume_id}"))
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            if detail.versions[0].status == ResumeStatus::Complete {
                let run = detail.versions[0]
                    .runs
                    .iter()
                    .find(|r| r.plugin_id == "indy_hackers_hiring")
                    .unwrap();
                assert!(run.parse.is_some());
                assert_eq!(
                    run.verdict
                        .as_ref()
                        .unwrap()
                        .report
                        .as_ref()
                        .unwrap()
                        .engine,
                    "Compact Intake"
                );
                complete = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert!(
            complete,
            "resume did not complete through signed Python callback"
        );
    }
    old_server.abort();
    server.abort();
}
