//! The real Python serializer and signature must survive Rust's typed contract.
use domain::{CallbackBody, ReportOutcome};

#[test]
#[ignore = "cross-runtime integration: run with PYTHON=<python3> cargo test -p domain --test python_report -- --ignored"]
fn python_reports_round_trip_through_the_signed_contract() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".into());
    let output = std::process::Command::new(python)
        .current_dir(root)
        .args([
            "-c",
            r#"
import json
from providers import stub
from providers.test_smart_resume import fixture
parsed = stub.read_file(stub.DOCX, fixture(True))
for item in stub.FLEET:
    raw = json.dumps({
        'run_id': '00000000-0000-0000-0000-000000000123',
        'parse': stub.as_parse(parsed),
        'verdict': stub.delivered_verdict(item, '00000000-0000-0000-0000-000000000123', parsed),
    }, separators=(',', ':'))
    print(json.dumps({'raw': raw, 'signature': stub.sign('report-test', 1700000000, raw.encode())}))
"#,
        ])
        .output()
        .expect("python3 provider serializer");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.lines().count(), 6);
    for line in output.lines() {
        let envelope: serde_json::Value = serde_json::from_str(line).unwrap();
        let raw = envelope["raw"].as_str().unwrap();
        domain::verify(
            "report-test",
            1_700_000_000,
            1_700_000_000,
            raw.as_bytes(),
            envelope["signature"].as_str().unwrap(),
        )
        .unwrap();
        let body: CallbackBody = serde_json::from_str(raw).unwrap();
        assert_eq!(
            body.verdict.report.as_ref().unwrap().outcome,
            ReportOutcome::ParsedWithWarnings
        );
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::from_str::<serde_json::Value>(raw).unwrap()
        );
        let mut legacy = serde_json::to_value(&body).unwrap();
        legacy["verdict"].as_object_mut().unwrap().remove("report");
        let old: CallbackBody = serde_json::from_value(legacy).unwrap();
        assert!(old.verdict.report.is_none());
        assert!(
            serde_json::to_value(old).unwrap()["verdict"]
                .get("report")
                .is_none()
        );
    }
}
