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

fn plain_docx() -> Vec<u8> {
    use std::io::Write;
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .expect("document entry");
    archive
        .write_all(b"<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t>Ada Lovelace</w:t></w:r></w:p></w:body></w:document>")
        .expect("document xml");
    archive.finish().expect("docx").into_inner()
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

    let resume_uuid = uuid::Uuid::parse_str(resume_id).unwrap();
    assert_eq!(
        store
            .detail(ada_uuid, resume_uuid)
            .await
            .unwrap()
            .unwrap()
            .title,
        "ada.pdf"
    );
    for (cookie, expected) in [
        (None, reqwest::StatusCode::UNAUTHORIZED),
        (Some(&grace_cookie), reqwest::StatusCode::NOT_FOUND),
    ] {
        let mut request = client
            .patch(format!("{base}/api/resumes/{resume_id}"))
            .json(&serde_json::json!({ "title": "Not yours" }));
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        assert_eq!(request.send().await.unwrap().status(), expected);
    }
    for title in ["   ".to_owned(), "x".repeat(201)] {
        let invalid = client
            .patch(format!("{base}/api/resumes/{resume_id}"))
            .header("cookie", &ada_cookie)
            .json(&serde_json::json!({ "title": title }))
            .send()
            .await
            .unwrap();
        assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    }
    assert_eq!(
        store
            .detail(ada_uuid, resume_uuid)
            .await
            .unwrap()
            .unwrap()
            .title,
        "ada.pdf"
    );
    let renamed = client
        .patch(format!("{base}/api/resumes/{resume_id}"))
        .header("cookie", &ada_cookie)
        .json(&serde_json::json!({ "title": "  Engineering roles  " }))
        .send()
        .await
        .unwrap();
    assert_eq!(renamed.status(), reqwest::StatusCode::NO_CONTENT);
    let detail = store.detail(ada_uuid, resume_uuid).await.unwrap().unwrap();
    assert_eq!(detail.title, "Engineering roles");
    assert_eq!(detail.versions[0].filename, "ada.pdf");
    assert_eq!(
        store.list(ada_uuid).await.unwrap()[0].title,
        "Engineering roles"
    );
    objects
        .get(&object_key)
        .await
        .expect("rename preserves original bytes");

    // This resume predates the current registry. An unowned detail request
    // must not enqueue work, and repeated owner visits add the new plugin once.
    let version_uuid = uuid::Uuid::parse_str(version_id).expect("version uuid");
    let parse = domain::ParseInput::empty(domain::LayoutConfidence::Inferred);
    store
        .save_parse(version_uuid, &parse, None)
        .await
        .expect("parse");
    store
        .fanout(version_uuid, &["legacy"])
        .await
        .expect("legacy run");
    let before = store
        .detail(ada_uuid, uuid::Uuid::parse_str(resume_id).unwrap())
        .await
        .unwrap()
        .unwrap();
    let legacy_id = before.versions[0].runs[0].id;
    let verdict = domain::Verdict {
        kept: vec![],
        dropped: vec![],
        score: 50,
        summary: "Score 50 of 100.".into(),
        factors: vec![],
        report: Some(domain::ProviderReport {
            engine: "Plaintext Classic".into(),
            engine_version: "3.8.2".into(),
            reference: "LEGACY-REPORT".into(),
            outcome: domain::ReportOutcome::Parsed,
            contact_fields: 2,
            sections_detected: 0,
            bullets_detected: 0,
            layout_confidence: domain::LayoutConfidence::Inferred,
            notes: vec!["Simulated parser report.".into()],
        }),
    };
    store
        .record_callback(
            "legacy",
            legacy_id,
            &legacy_id.to_string(),
            &parse,
            &verdict,
        )
        .await
        .expect("legacy callback");
    let hidden = client
        .get(format!("{base}/api/resumes/{resume_id}"))
        .header("cookie", &grace_cookie)
        .send()
        .await
        .expect("hidden completed resume");
    assert_eq!(hidden.status(), reqwest::StatusCode::NOT_FOUND);
    let count: i64 = sqlx::query_scalar("select count(*) from runs where version_id = $1")
        .bind(version_uuid)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    for _ in 0..2 {
        let detail: serde_json::Value = client
            .get(format!("{base}/api/resumes/{resume_id}"))
            .header("cookie", &ada_cookie)
            .send()
            .await
            .expect("owner detail")
            .error_for_status()
            .expect("detail ok")
            .json()
            .await
            .expect("detail json");
        assert_eq!(detail["versions"][0]["status"], "running");
        assert_eq!(detail["versions"][0]["runs"].as_array().unwrap().len(), 2);
        let legacy_run = detail["versions"][0]["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|run| run["plugin_id"] == "legacy")
            .unwrap();
        assert_eq!(
            legacy_run["verdict"]["report"],
            serde_json::to_value(&verdict.report).unwrap()
        );
    }
    let requested: i64 = sqlx::query_scalar("select count(*) from outbox where version_id = $1 and name = 'AtsRunRequested' and payload->>'plugin_id' = 'stub'")
        .bind(version_uuid).fetch_one(store.pool()).await.unwrap();
    assert_eq!(requested, 1);
    let legacy: serde_json::Value = sqlx::query_scalar("select verdict from runs where id = $1")
        .bind(legacy_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(legacy["score"], 50);

    let file_url = format!("{base}/api/resumes/{resume_id}/versions/{version_id}/file");
    let anonymous_file = client.get(&file_url).send().await.expect("anonymous file");
    assert_eq!(anonymous_file.status(), reqwest::StatusCode::UNAUTHORIZED);
    let hidden_file = client
        .get(&file_url)
        .header("cookie", &grace_cookie)
        .send()
        .await
        .expect("hidden file");
    assert_eq!(hidden_file.status(), reqwest::StatusCode::NOT_FOUND);
    let original = client
        .get(&file_url)
        .header("cookie", &ada_cookie)
        .send()
        .await
        .expect("original file");
    assert_eq!(original.status(), reqwest::StatusCode::OK);
    assert_eq!(original.headers()["content-type"], domain::PDF);
    assert_eq!(original.headers()["cache-control"], "private, no-store");
    assert_eq!(original.headers()["x-content-type-options"], "nosniff");
    assert_eq!(
        original.bytes().await.expect("file bytes").as_ref(),
        plain_pdf()
    );

    let wrong_resume = client
        .get(format!(
            "{base}/api/resumes/{}/versions/{version_id}/file",
            uuid::Uuid::new_v4()
        ))
        .header("cookie", &ada_cookie)
        .send()
        .await
        .expect("wrong resume");
    assert_eq!(wrong_resume.status(), reqwest::StatusCode::NOT_FOUND);
    let missing_version = client
        .get(format!(
            "{base}/api/resumes/{resume_id}/versions/{}/file",
            uuid::Uuid::new_v4()
        ))
        .header("cookie", &ada_cookie)
        .send()
        .await
        .expect("missing version");
    assert_eq!(missing_version.status(), reqwest::StatusCode::NOT_FOUND);

    // A new DOCX version must not change the original PDF's file endpoint.
    let docx = plain_docx();
    let (content_type, body) = multipart(&docx);
    let new_version: serde_json::Value = client
        .post(format!("{base}/api/resumes/{resume_id}/versions"))
        .header("cookie", &ada_cookie)
        .header("content-type", content_type)
        .body(body)
        .send()
        .await
        .expect("docx upload")
        .error_for_status()
        .expect("docx accepted")
        .json()
        .await
        .expect("docx version");
    let docx_id = new_version["version_id"].as_str().expect("docx id");
    assert_eq!(
        store
            .detail(ada_uuid, resume_uuid)
            .await
            .unwrap()
            .unwrap()
            .title,
        "Engineering roles"
    );
    let docx_url = format!("{base}/api/resumes/{resume_id}/versions/{docx_id}/file");
    let original_docx = client
        .get(&docx_url)
        .header("cookie", &ada_cookie)
        .send()
        .await
        .expect("original docx");
    assert_eq!(original_docx.status(), reqwest::StatusCode::OK);
    assert_eq!(original_docx.headers()["content-type"], domain::DOCX);
    assert_eq!(
        original_docx.headers()["content-disposition"],
        "inline; filename=\"resume.docx\""
    );
    assert_eq!(
        original_docx.bytes().await.expect("docx bytes").as_ref(),
        docx
    );
    let old_pdf = client
        .get(&file_url)
        .header("cookie", &ada_cookie)
        .send()
        .await
        .expect("old pdf");
    assert_eq!(old_pdf.headers()["content-type"], domain::PDF);
    assert_eq!(
        old_pdf.bytes().await.expect("pdf bytes").as_ref(),
        plain_pdf()
    );
    let hidden_docx = client
        .get(&docx_url)
        .header("cookie", &grace_cookie)
        .send()
        .await
        .expect("hidden docx");
    assert_eq!(hidden_docx.status(), reqwest::StatusCode::NOT_FOUND);
    // Resume deletion is scoped to the owner and cleans up the uploaded objects.
    let (content_type, body) = multipart(&plain_pdf());
    let extra: serde_json::Value = client
        .post(format!("{base}/api/resumes"))
        .header("cookie", &ada_cookie)
        .header("content-type", content_type)
        .body(body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let extra_id = extra["id"].as_str().unwrap();
    let first_id = extra["version_id"].as_str().unwrap();
    let (content_type, body) = multipart(&plain_pdf());
    let revised: serde_json::Value = client
        .post(format!("{base}/api/resumes/{extra_id}/versions"))
        .header("cookie", &ada_cookie)
        .header("content-type", content_type)
        .body(body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let revised_id = revised["version_id"].as_str().unwrap();
    let revised_key = domain::version_object_key(
        ada_uuid,
        extra_id.parse().unwrap(),
        revised_id.parse().unwrap(),
    );
    let first_key = domain::version_object_key(
        ada_uuid,
        extra_id.parse().unwrap(),
        first_id.parse().unwrap(),
    );
    for path in [
        format!("/api/resumes/{extra_id}"),
        format!("/api/resumes/{extra_id}/versions/{revised_id}"),
    ] {
        assert_eq!(
            client
                .delete(format!("{base}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client
                .delete(format!("{base}{path}"))
                .header("cookie", &grace_cookie)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        client
            .delete(format!(
                "{base}/api/resumes/{resume_id}/versions/{revised_id}"
            ))
            .header("cookie", &ada_cookie)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    assert_eq!(
        client
            .delete(format!(
                "{base}/api/resumes/{extra_id}/versions/{revised_id}"
            ))
            .header("cookie", &ada_cookie)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NO_CONTENT
    );
    assert!(objects.get(&revised_key).await.is_err());
    objects.get(&first_key).await.unwrap();
    let detail = store
        .detail(ada_uuid, extra_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detail.versions.len(), 1);
    assert_eq!(
        client
            .delete(format!("{base}/api/resumes/{extra_id}"))
            .header("cookie", &ada_cookie)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NO_CONTENT
    );
    assert!(objects.get(&first_key).await.is_err());
    assert!(
        store
            .detail(ada_uuid, extra_id.parse().unwrap())
            .await
            .unwrap()
            .is_none()
    );

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
