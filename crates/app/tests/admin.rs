//! Admin feedback triage: same session cookie, different authorization.

use std::sync::Arc;

use app::{Api, Bus, api_router, compiled_plugins};
use domain::{
    AnalyzeFeedbackPayload, DetectedSection, PROFILE_SUMMARY_ID, ParseInput, ProfileSummary,
    ResumeMarkdown, SectionKind, SectionMap, SectionVote,
};
use storage::MemoryObjects;
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

fn session_pair(response: &reqwest::Response) -> String {
    let header = response
        .headers()
        .get("set-cookie")
        .expect("set-cookie")
        .to_str()
        .expect("cookie ascii");
    header.split(';').next().expect("cookie pair").to_owned()
}

async fn sign_in(base: &str, store: &storage::Store, email: &str) -> (String, String) {
    let now = auth::OffsetDateTime::now_utc();
    let issued = auth::request_magic_link(store.pool(), email, now, None)
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
    (cookie, user_id)
}

#[tokio::test]
async fn admins_triage_downvoted_feedback_without_deleting_it() {
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
        admin_email_domains: vec!["admin.example.com".to_owned()],
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

    let (admin_cookie, admin_id) = sign_in(&base, &store, "ada@admin.example.com").await;
    let (user_cookie, _) = sign_in(&base, &store, "grace@example.com").await;
    let admin_uuid: uuid::Uuid = admin_id.parse().expect("admin uuid");

    let me: serde_json::Value = client
        .get(format!("{base}/api/me"))
        .header("cookie", &admin_cookie)
        .send()
        .await
        .expect("me")
        .json()
        .await
        .expect("me json");
    assert_eq!(me["admin"], true);
    let other_me: serde_json::Value = client
        .get(format!("{base}/api/me"))
        .header("cookie", &user_cookie)
        .send()
        .await
        .expect("other me")
        .json()
        .await
        .expect("other me json");
    assert_eq!(other_me["admin"], false);

    let resume_id = uuid::Uuid::new_v4();
    let version_id = uuid::Uuid::new_v4();
    store
        .create_resume(
            admin_uuid,
            resume_id,
            version_id,
            "ada.pdf",
            domain::PDF,
            "file",
        )
        .await
        .expect("resume");
    let parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
    let sections = SectionMap {
        engine: "sections".into(),
        model: "bge".into(),
        sections: vec![DetectedSection {
            id: "experience".into(),
            kind: SectionKind::Experience,
            heading: "Experience".into(),
            body: "Built small systems.".into(),
            confidence: 0.9,
            feedback: None,
        }],
    };
    let summary = ProfileSummary {
        text: "A builder of small systems.".into(),
        engine: "extractive".into(),
        model: "summary".into(),
        feedback: None,
    };
    store
        .save_parse(version_id, &parse, Some(&sections), Some(&summary))
        .await
        .expect("parse");
    for section_id in ["experience", PROFILE_SUMMARY_ID] {
        assert!(
            store
                .vote_section(
                    admin_uuid,
                    resume_id,
                    version_id,
                    section_id,
                    Some(SectionVote::Down),
                )
                .await
                .expect("vote")
        );
    }
    let experience = store
        .save_feedback_analysis(
            &AnalyzeFeedbackPayload {
                version_id,
                user_id: admin_uuid,
                section_id: "experience".into(),
                section_heading: "Experience".into(),
                section_kind: SectionKind::Experience,
            },
            "llama.cpp",
            "llama-3.2",
            "The heading is present, but the body reads as a fragment.",
        )
        .await
        .expect("analysis");
    let profile = store
        .save_feedback_analysis(
            &AnalyzeFeedbackPayload {
                version_id,
                user_id: admin_uuid,
                section_id: PROFILE_SUMMARY_ID.into(),
                section_heading: "Profile impression".into(),
                section_kind: SectionKind::Summary,
            },
            "extractive",
            "summary",
            "The impression is too generic.",
        )
        .await
        .expect("profile analysis");

    assert_eq!(
        client
            .get(format!("{base}/api/admin/feedback"))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .get(format!("{base}/api/admin/feedback"))
            .header("cookie", &user_cookie)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .put(format!(
                "{base}/api/admin/feedback/analyses/{}",
                experience.id
            ))
            .header("cookie", &user_cookie)
            .json(&serde_json::json!({ "handled": true }))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );

    let listed: serde_json::Value = client
        .get(format!("{base}/api/admin/feedback"))
        .header("cookie", &admin_cookie)
        .send()
        .await
        .expect("admin feedback")
        .error_for_status()
        .expect("admin ok")
        .json()
        .await
        .expect("admin json");
    let feedback = listed["feedback"].as_array().expect("feedback");
    assert_eq!(feedback.len(), 2);
    let experience_item = feedback
        .iter()
        .find(|item| item["section_id"] == "experience")
        .expect("experience feedback");
    assert_eq!(experience_item["user_email"], "ada@admin.example.com");
    assert_eq!(experience_item["resume_title"], "ada.pdf");
    assert_eq!(experience_item["version_number"], 1);
    assert_eq!(experience_item["section_heading"], "Experience");
    assert_eq!(experience_item["section_kind"], "experience");
    assert_eq!(experience_item["section_body"], "Built small systems.");
    assert_eq!(experience_item["analyses"].as_array().unwrap().len(), 1);
    assert_eq!(
        experience_item["analyses"][0]["diagnosis"],
        "The heading is present, but the body reads as a fragment."
    );
    assert!(experience_item["analyses"][0]["handled_at"].is_null());
    let profile_item = feedback
        .iter()
        .find(|item| item["section_id"] == PROFILE_SUMMARY_ID)
        .expect("profile feedback");
    assert_eq!(profile_item["section_heading"], "Profile impression");
    assert_eq!(profile_item["section_kind"], "summary");
    assert_eq!(profile_item["section_body"], "A builder of small systems.");

    let handled = client
        .put(format!(
            "{base}/api/admin/feedback/analyses/{}",
            experience.id
        ))
        .header("cookie", &admin_cookie)
        .json(&serde_json::json!({ "handled": true }))
        .send()
        .await
        .expect("handle");
    assert_eq!(handled.status(), reqwest::StatusCode::NO_CONTENT);
    let (is_handled, handled_by): (bool, Option<String>) = sqlx::query_as(
        "select handled_at is not null, handled_by from feedback_analysis where id = $1",
    )
    .bind(experience.id)
    .fetch_one(store.pool())
    .await
    .expect("handled row");
    assert!(is_handled);
    assert_eq!(handled_by.as_deref(), Some("ada@admin.example.com"));
    let rows: i64 = sqlx::query_scalar("select count(*) from feedback_analysis")
        .fetch_one(store.pool())
        .await
        .expect("count");
    assert_eq!(rows, 2, "handling never deletes the analysis");

    let listed: serde_json::Value = client
        .get(format!("{base}/api/admin/feedback"))
        .header("cookie", &admin_cookie)
        .send()
        .await
        .expect("admin feedback after handle")
        .json()
        .await
        .expect("admin json after handle");
    let handled_item = listed["feedback"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["section_id"] == "experience")
        .unwrap();
    assert!(handled_item["analyses"][0]["handled_at"].is_string());
    assert_eq!(
        handled_item["analyses"][0]["handled_by"],
        "ada@admin.example.com"
    );

    let reopened = client
        .put(format!(
            "{base}/api/admin/feedback/analyses/{}",
            experience.id
        ))
        .header("cookie", &admin_cookie)
        .json(&serde_json::json!({ "handled": false }))
        .send()
        .await
        .expect("reopen");
    assert_eq!(reopened.status(), reqwest::StatusCode::NO_CONTENT);
    let (is_handled, handled_by): (bool, Option<String>) = sqlx::query_as(
        "select handled_at is not null, handled_by from feedback_analysis where id = $1",
    )
    .bind(experience.id)
    .fetch_one(store.pool())
    .await
    .expect("reopened row");
    assert!(!is_handled);
    assert_eq!(handled_by, None);

    assert_eq!(
        client
            .put(format!(
                "{base}/api/admin/feedback/analyses/{}",
                uuid::Uuid::new_v4()
            ))
            .header("cookie", &admin_cookie)
            .json(&serde_json::json!({ "handled": true }))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );

    // Rerunning drops the stored diagnosis and asks the worker to regenerate
    // it. The worker is not running in this test, so the row is gone and an
    // outbox event is enqueued; the downvote itself stays.
    store
        .save_resume_markdown(
            version_id,
            &ResumeMarkdown {
                text: "# Ada\n\n## Experience\n\nBuilt analytical engines.".to_owned(),
                engine: "test".to_owned(),
                model: "alias".to_owned(),
            },
        )
        .await
        .expect("markdown");
    let rerun = client
        .post(format!(
            "{base}/api/admin/feedback/analyses/{}",
            experience.id
        ))
        .header("cookie", &admin_cookie)
        .send()
        .await
        .expect("rerun");
    assert_eq!(rerun.status(), reqwest::StatusCode::ACCEPTED);
    let remaining: i64 = sqlx::query_scalar("select count(*) from feedback_analysis")
        .fetch_one(store.pool())
        .await
        .expect("count after rerun");
    assert_eq!(remaining, 1, "rerun deletes the diagnosis it regenerates");
    let pending: i64 = sqlx::query_scalar(
        "select count(*) from outbox where name = 'AnalyzeFeedback' and dispatched_at is null",
    )
    .fetch_one(store.pool())
    .await
    .expect("pending outbox");
    assert_eq!(pending, 1, "rerun enqueues a fresh diagnosis");
    assert_eq!(
        client
            .post(format!(
                "{base}/api/admin/feedback/analyses/{}",
                uuid::Uuid::new_v4()
            ))
            .header("cookie", &admin_cookie)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );

    // A later up vote hides the item from the admin list but keeps the
    // analysis row and its handling history.
    assert!(
        store
            .vote_section(
                admin_uuid,
                resume_id,
                version_id,
                "experience",
                Some(SectionVote::Up),
            )
            .await
            .unwrap()
    );
    let listed: serde_json::Value = client
        .get(format!("{base}/api/admin/feedback"))
        .header("cookie", &admin_cookie)
        .send()
        .await
        .expect("admin feedback after up vote")
        .json()
        .await
        .expect("admin json after up vote");
    assert_eq!(listed["feedback"].as_array().unwrap().len(), 1);
    assert_eq!(listed["feedback"][0]["section_id"], PROFILE_SUMMARY_ID);
    let rows: i64 = sqlx::query_scalar("select count(*) from feedback_analysis")
        .fetch_one(store.pool())
        .await
        .expect("count after up vote");
    assert_eq!(
        rows, 1,
        "rerun deleted the experience diagnosis; the profile one stays"
    );
    let _ = profile.id;
}
