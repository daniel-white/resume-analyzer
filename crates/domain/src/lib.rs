//! The shapes in `docs/contracts.md`. Plugin and HTTP code serialize these
//! types directly so a second shape cannot sneak in beside the contract.

use chrono::{DateTime, Utc};
use hmac::Mac;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PDF: &str = "application/pdf";
pub const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
pub const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;
pub const SKEW_SECONDS: i64 = 300;

/// Owner used by storage tests that never open a session.
pub const DEMO_USER_ID: Uuid = Uuid::from_u128(1);

pub fn version_object_key(user_id: Uuid, resume_id: Uuid, version_id: Uuid) -> String {
    format!("users/{user_id}/resumes/{resume_id}/versions/{version_id}")
}

/// The bytes a plugin reads. Vendors do not share a parse.
pub struct ResumeFile<'a> {
    pub media_type: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VendorRead {
    pub parse: ParseInput,
    pub verdict: Verdict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseInput {
    pub profile: Profile,
    pub layout: Layout,
    pub layout_confidence: LayoutConfidence,
}

impl ParseInput {
    pub fn empty(layout_confidence: LayoutConfidence) -> Self {
        Self {
            profile: Profile {
                name: None,
                email: None,
                phone: None,
                sections: Vec::new(),
            },
            layout: Layout {
                columns: 1,
                contact_in_header: false,
                has_tables: false,
                has_text_boxes: false,
                has_images: false,
                bullet_count: 0,
                dates: DateStyle::Absent,
            },
            layout_confidence,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub heading: String,
    pub body: String,
    pub in_table: bool,
    pub in_text_box: bool,
    pub in_header: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub columns: u32,
    pub contact_in_header: bool,
    pub has_tables: bool,
    pub has_text_boxes: bool,
    pub has_images: bool,
    pub bullet_count: u32,
    pub dates: DateStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutConfidence {
    Structural,
    Inferred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DateStyle {
    MmYyyy,
    Other,
    Absent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResumeStatus {
    Uploaded,
    Parsed,
    Running,
    Complete,
}

impl ResumeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Uploaded => "uploaded",
            Self::Parsed => "parsed",
            Self::Running => "running",
            Self::Complete => "complete",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "uploaded" => Self::Uploaded,
            "parsed" => Self::Parsed,
            "running" => Self::Running,
            "complete" => Self::Complete,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Requested,
    Complete,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Complete => "complete",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "requested" => Self::Requested,
            "complete" => Self::Complete,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventName {
    ResumeUploaded,
    ResumeParsed,
    AtsRunRequested,
    AtsCallbackReceived,
}

impl EventName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResumeUploaded => "ResumeUploaded",
            Self::ResumeParsed => "ResumeParsed",
            Self::AtsRunRequested => "AtsRunRequested",
            Self::AtsCallbackReceived => "AtsCallbackReceived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "ResumeUploaded" => Self::ResumeUploaded,
            "ResumeParsed" => Self::ResumeParsed,
            "AtsRunRequested" => Self::AtsRunRequested,
            "AtsCallbackReceived" => Self::AtsCallbackReceived,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: Uuid,
    pub resume_id: Uuid,
    pub version_id: Uuid,
    pub name: EventName,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadedPayload {
    pub user_id: Uuid,
    pub resume_id: Uuid,
    pub version_id: Uuid,
    pub object_key: String,
    pub filename: String,
    pub media_type: String,
}

/// The app's verification extract. Not a plugin input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParsedPayload {
    pub parse: ParseInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPayload {
    pub run_id: Uuid,
    pub plugin_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Factor {
    pub label: String,
    pub delta: i32,
    /// One sentence. Negative factors say what to change. Positive factors say what to keep.
    pub advice: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub kept: Vec<String>,
    pub dropped: Vec<String>,
    pub score: u8,
    /// Template of the factor advice, changes first. Not a model call.
    pub summary: String,
    pub factors: Vec<Factor>,
}

/// `clamp(base + sum(delta), 0, 100)`. Plugins pick the base.
pub fn aggregate(base: i32, factors: &[Factor]) -> u8 {
    let sum = factors
        .iter()
        .fold(base, |acc, factor| acc.saturating_add(factor.delta));
    sum.clamp(0, 100) as u8
}

/// Score, then what to change, then what helped. Empty advice is skipped.
pub fn summarize(score: u8, factors: &[Factor]) -> String {
    let fixes: Vec<&str> = factors
        .iter()
        .filter(|factor| factor.delta < 0 && !factor.advice.is_empty())
        .map(|factor| factor.advice.as_str())
        .collect();
    let helped: Vec<&str> = factors
        .iter()
        .filter(|factor| factor.delta > 0 && !factor.advice.is_empty())
        .map(|factor| factor.advice.as_str())
        .collect();
    let mut text = format!("Score {score} of 100.");
    if fixes.is_empty() && helped.is_empty() {
        text.push_str(" Nothing to change for this system.");
        return text;
    }
    if !fixes.is_empty() {
        text.push(' ');
        text.push_str(&fixes.join(" "));
    }
    if !helped.is_empty() {
        text.push(' ');
        text.push_str(&helped.join(" "));
    }
    text
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallbackBody {
    pub run_id: Uuid,
    pub parse: ParseInput,
    pub verdict: Verdict,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CallbackPayload {
    pub run_id: Uuid,
    pub plugin_id: String,
    pub parse: ParseInput,
    pub verdict: Verdict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionSummary {
    pub id: Uuid,
    pub number: i32,
    pub filename: String,
    pub status: ResumeStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeSummary {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub latest: VersionSummary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunView {
    pub id: Uuid,
    pub plugin_id: String,
    pub status: RunStatus,
    pub parse: Option<ParseInput>,
    pub verdict: Option<Verdict>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VersionDetail {
    pub id: Uuid,
    pub number: i32,
    pub filename: String,
    pub media_type: String,
    pub status: ResumeStatus,
    pub created_at: DateTime<Utc>,
    /// The app's verification extract for this upload. Distinct from each run's parse.
    pub parse: Option<ParseInput>,
    pub runs: Vec<RunView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResumeDetail {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    /// Newest version first, so the latest upload is the one on top.
    pub versions: Vec<VersionDetail>,
}

/// Compiled-in integrations. `read` opens the stored file. The worker posts
/// that result as a signed callback and does not store the return value.
/// Only the callback becomes a run result. Plugins do not share a parse.
pub trait Ats: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn subtitle(&self) -> &'static str;
    fn secret(&self) -> &str;
    fn read(&self, file: &ResumeFile<'_>) -> VendorRead;
}

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

pub fn sign(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureError {
    Skew,
    Signature,
}

pub fn verify(
    secret: &str,
    now: i64,
    timestamp: i64,
    body: &[u8],
    signature: &str,
) -> Result<(), SignatureError> {
    if (now - timestamp).abs() > SKEW_SECONDS {
        return Err(SignatureError::Skew);
    }
    let provided = hex::decode(signature).map_err(|_| SignatureError::Signature)?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);
    mac.verify_slice(&provided)
        .map_err(|_| SignatureError::Signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names_match_the_contract() {
        for name in [
            EventName::ResumeUploaded,
            EventName::ResumeParsed,
            EventName::AtsRunRequested,
            EventName::AtsCallbackReceived,
        ] {
            let json = serde_json::to_value(name).unwrap();
            assert_eq!(json, name.as_str());
            assert_eq!(EventName::parse(name.as_str()), Some(name));
        }
        assert_eq!(EventName::parse("resume_uploaded"), None);
    }

    #[test]
    fn score_clamps_to_the_range() {
        let up = [Factor {
            label: "headings".into(),
            delta: 80,
            advice: String::new(),
        }];
        let down = [Factor {
            label: "table".into(),
            delta: -80,
            advice: String::new(),
        }];
        assert_eq!(aggregate(50, &up), 100);
        assert_eq!(aggregate(10, &down), 0);
        assert_eq!(
            aggregate(
                40,
                &[
                    Factor {
                        label: "a".into(),
                        delta: 15,
                        advice: String::new(),
                    },
                    Factor {
                        label: "b".into(),
                        delta: -5,
                        advice: String::new(),
                    },
                ],
            ),
            50
        );
    }

    #[test]
    fn upload_limit_is_five_mebibytes() {
        assert_eq!(MAX_UPLOAD_BYTES, 5_242_880);
    }

    #[test]
    fn object_keys_are_scoped_to_the_user_and_version() {
        let user = Uuid::from_u128(1);
        let resume = Uuid::from_u128(2);
        let version = Uuid::from_u128(3);
        assert_eq!(DEMO_USER_ID, user);
        assert_eq!(
            version_object_key(user, resume, version),
            "users/00000000-0000-0000-0000-000000000001/resumes/00000000-0000-0000-0000-000000000002/versions/00000000-0000-0000-0000-000000000003"
        );
    }

    #[test]
    fn summary_lists_changes_before_what_helped() {
        let factors = [
            Factor {
                label: "table".into(),
                delta: -20,
                advice: "Replace the table with plain lines of text.".into(),
            },
            Factor {
                label: "headings".into(),
                delta: 10,
                advice: "Keep the standard headings.".into(),
            },
        ];
        let score = aggregate(50, &factors);
        assert_eq!(
            summarize(score, &factors),
            "Score 40 of 100. Replace the table with plain lines of text. Keep the standard headings."
        );
        assert_eq!(
            summarize(50, &[]),
            "Score 50 of 100. Nothing to change for this system."
        );
    }

    #[test]
    fn signature_covers_the_raw_body() {
        let body = br#"{"run_id":"x"}"#;
        let signature = sign("secret", 1_700_000_000, body);
        assert!(verify("secret", 1_700_000_100, 1_700_000_000, body, &signature).is_ok());
        assert_eq!(
            verify(
                "secret",
                1_700_000_000,
                1_700_000_000,
                b"tampered",
                &signature
            ),
            Err(SignatureError::Signature)
        );
        assert_eq!(
            verify("secret", 1_700_000_301, 1_700_000_000, body, &signature),
            Err(SignatureError::Skew)
        );
    }

    #[test]
    fn date_style_serializes_as_mm_yyyy() {
        assert_eq!(serde_json::to_value(DateStyle::MmYyyy).unwrap(), "mm-yyyy");
    }
}
