//! The shapes in `docs/contracts.md`. Plugin and HTTP code serialize these
//! types directly so a second shape cannot sneak in beside the contract.

use chrono::{DateTime, Utc};
use hmac::{KeyInit, Mac};
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
    /// Build the version's markdown rendering, once after the first thumbs down.
    GenerateMarkdown,
    /// Diagnose one thumbs down with the markdown and the original resume.
    AnalyzeFeedback,
}

impl EventName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResumeUploaded => "ResumeUploaded",
            Self::ResumeParsed => "ResumeParsed",
            Self::AtsRunRequested => "AtsRunRequested",
            Self::AtsCallbackReceived => "AtsCallbackReceived",
            Self::GenerateMarkdown => "GenerateMarkdown",
            Self::AnalyzeFeedback => "AnalyzeFeedback",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "ResumeUploaded" => Self::ResumeUploaded,
            "ResumeParsed" => Self::ResumeParsed,
            "AtsRunRequested" => Self::AtsRunRequested,
            "AtsCallbackReceived" => Self::AtsCallbackReceived,
            "GenerateMarkdown" => Self::GenerateMarkdown,
            "AnalyzeFeedback" => Self::AnalyzeFeedback,
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
    /// Optional simulated provider diagnostics. Older stored verdicts omit this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<ProviderReport>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderReport {
    pub engine: String,
    pub engine_version: String,
    pub reference: String,
    pub outcome: ReportOutcome,
    pub contact_fields: u32,
    pub sections_detected: u32,
    pub bullets_detected: u32,
    pub layout_confidence: LayoutConfidence,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportOutcome {
    Parsed,
    ParsedWithWarnings,
    Unreadable,
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

/// Transient extraction metadata; formatting is evidence, not a section label.
#[derive(Clone, Debug, Default)]
pub struct DocumentLine {
    pub text: String,
    pub emphasized: bool,
    pub font_size: Option<f32>,
}

/// Canonical section labels. The detector maps free-text headings onto these
/// so the UI can compare our reading with each provider's extract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    Summary,
    Experience,
    Education,
    Skills,
    Projects,
    Certifications,
    Awards,
    Languages,
    Publications,
    Volunteer,
    Interests,
    Contact,
    Other,
}

impl SectionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Experience => "Experience",
            Self::Education => "Education",
            Self::Skills => "Skills",
            Self::Projects => "Projects",
            Self::Certifications => "Certifications",
            Self::Awards => "Awards",
            Self::Languages => "Languages",
            Self::Publications => "Publications",
            Self::Volunteer => "Volunteer",
            Self::Interests => "Interests",
            Self::Contact => "Contact",
            Self::Other => "Other",
        }
    }

    /// Read the stored snake_case form. Unknown labels become `Other` so a
    /// stored feedback row never fails to load.
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "summary" => Self::Summary,
            "experience" => Self::Experience,
            "education" => Self::Education,
            "skills" => Self::Skills,
            "projects" => Self::Projects,
            "certifications" => Self::Certifications,
            "awards" => Self::Awards,
            "languages" => Self::Languages,
            "publications" => Self::Publications,
            "volunteer" => Self::Volunteer,
            "interests" => Self::Interests,
            "contact" => Self::Contact,
            "other" => Self::Other,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionVote {
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetectedSection {
    /// Stable within one version, so feedback can point at it.
    pub id: String,
    pub kind: SectionKind,
    pub heading: String,
    pub body: String,
    pub confidence: f32,
    /// The calling user's vote. Not stored inside the section map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<SectionVote>,
}

/// One model's reading of the resume. Distinct from ParseInput: this is our
/// representation, not a vendor's extraction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionMap {
    pub engine: String,
    pub model: String,
    pub sections: Vec<DetectedSection>,
}

pub const PROFILE_SUMMARY_ID: &str = "profile-summary";

/// A model-generated description of the candidate, not an ATS score summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub text: String,
    pub engine: String,
    pub model: String,
    /// The calling user's vote. Not stored inside the generated summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<SectionVote>,
}

/// A markdown rendering of the detected sections. Built once per version,
/// after the first thumbs down, so a feedback analysis reads the same sections
/// the user saw instead of re-parsing the file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeMarkdown {
    pub text: String,
    pub engine: String,
    pub model: String,
}

/// Build a version's markdown rendering. Enqueued once per version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateMarkdownPayload {
    pub version_id: Uuid,
}

/// Diagnose one rejected reading. The worker pairs this with the stored
/// markdown and the original resume text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzeFeedbackPayload {
    pub version_id: Uuid,
    pub user_id: Uuid,
    pub section_id: String,
    pub section_heading: String,
    pub section_kind: SectionKind,
}

/// One model diagnosis for a single thumbs down. A later thumbs down on the
/// same version stores a new row; the history is kept so a user can see every
/// diagnosis the model gave for every item they rejected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackAnalysis {
    pub id: Uuid,
    pub version_id: Uuid,
    pub user_id: Uuid,
    pub section_id: String,
    pub section_heading: String,
    pub section_kind: SectionKind,
    pub model: String,
    pub engine: String,
    pub diagnosis: String,
    pub created_at: DateTime<Utc>,
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
    /// The app's section detector reading, when it has run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sections: Option<SectionMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_summary: Option<ProfileSummary>,
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

/// Compiled-in integrations. `read` opens the stored file. The caller invokes
/// the plugin with that file and the webhook URL, and the plugin posts a
/// signed callback. The return value is not stored. Only the callback becomes
/// a run result. Plugins do not share a parse.
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
