//! Simulated ATS integrations. The spine still compiles in `stub` so an upload
//! can reach one callback without the Python providers. `fleet` is Greenline,
//! Levr, Workdaye, Ash, and SmartResume. The running API posts those files to the Python
//! fake and does not call `read` or `call`. `read` is what the fleet tests
//! call. The factor lists disagree.
//!
//! A plugin does not return the verdict to the worker. `call` signs a callback
//! and posts it. The fleet leaves that to the Python process.

use std::io::{Cursor, Read};
use std::sync::{Arc, LazyLock};

use domain::{
    Ats, DateStyle, Factor, LayoutConfidence, ParseInput, ResumeFile, Section, VendorRead, Verdict,
    aggregate, summarize,
};
use zip::ZipArchive;

mod ash;
mod greenline;
mod levr;
mod smart_resume;
mod workdaye;

pub use ash::Ash;
pub use greenline::Greenline;
pub use levr::Levr;
pub use smart_resume::SmartResume;
pub use workdaye::Workdaye;

pub struct Stub {
    secret: String,
}

impl Stub {
    pub const BASE: i32 = 50;

    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl Ats for Stub {
    fn id(&self) -> &'static str {
        "stub"
    }

    fn name(&self) -> &'static str {
        "Stub"
    }

    fn subtitle(&self) -> &'static str {
        "Stand-in parser. The simulated fleet replaces this."
    }

    fn secret(&self) -> &str {
        &self.secret
    }

    fn read(&self, file: &ResumeFile<'_>) -> VendorRead {
        let (parse, unreadable) = extract(file);
        VendorRead {
            verdict: score(&parse, unreadable),
            parse,
        }
    }
}

/// The app's first read, shown so the user can check the file came apart into
/// a name, contact, and sections. Plugins do not receive this value. Each one
/// calls `Ats::read` on the stored bytes.
pub fn structure(file: &ResumeFile<'_>) -> ParseInput {
    extract(file).0
}

/// Greenline, Levr, Workdaye, Ash, and SmartResume. The worker posts each file to the
/// matching Python provider instead of calling `read`.
pub fn fleet(secret: String) -> Vec<Arc<dyn Ats>> {
    vec![
        Arc::new(Greenline::new(secret.clone())),
        Arc::new(Levr::new(secret.clone())),
        Arc::new(Workdaye::new(secret.clone())),
        Arc::new(Ash::new(secret.clone())),
        Arc::new(SmartResume::new(secret)),
    ]
}

/// Call one plugin with the raw file and the webhook that receives its score.
/// The parse and verdict exist only long enough to sign the callback. The
/// caller does not keep them.
pub async fn call(
    plugin: &dyn Ats,
    file: &ResumeFile<'_>,
    webhook_url: &str,
    run_id: uuid::Uuid,
) -> anyhow::Result<()> {
    let read = plugin.read(file);
    let body = domain::CallbackBody {
        run_id,
        parse: read.parse,
        verdict: read.verdict,
    };
    let raw = serde_json::to_vec(&body)?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| anyhow::anyhow!("clock is before the unix epoch"))?
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX);
    let signature = domain::sign(plugin.secret(), timestamp, &raw);
    let response = http()
        .post(webhook_url)
        .header("content-type", "application/json")
        .header("x-timestamp", timestamp.to_string())
        .header("x-signature", signature)
        .header("idempotency-key", run_id.to_string())
        .body(raw)
        .send()
        .await?;
    anyhow::ensure!(
        response.status().is_success(),
        "callback for {run_id} returned {}",
        response.status()
    );
    Ok(())
}

fn http() -> &'static reqwest::Client {
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);
    &CLIENT
}

pub(crate) fn extract(file: &ResumeFile<'_>) -> (ParseInput, bool) {
    match file.media_type {
        domain::DOCX => match read_docx(file.bytes) {
            Some(parse) => (parse, false),
            None => (ParseInput::empty(LayoutConfidence::Structural), true),
        },
        domain::PDF if file.bytes.starts_with(b"%PDF") => (read_pdf(file.bytes), false),
        _ => (ParseInput::empty(LayoutConfidence::Inferred), true),
    }
}

fn score(input: &ParseInput, unreadable: bool) -> Verdict {
    if unreadable {
        return unreadable_verdict(Stub::BASE);
    }

    let mut marks = Marks::new();

    if input.layout.contact_in_header {
        marks.lose("header contact");
        marks.factor(
            "contact in header",
            -25,
            "Move the email and phone into the body, not the header.",
        );
    } else if input.profile.email.is_some() {
        marks.keep("email");
        marks.factor("email in body", 15, "Keep the email in the body.");
    }

    if input.layout.has_tables {
        marks.lose("table");
        marks.factor("table", -20, "Replace the table with plain lines of text.");
    }

    for section in &input.profile.sections {
        if !section.heading.is_empty() {
            marks.keep(section.heading.clone());
            marks.factor("standard headings", 10, "Keep the standard headings.");
        }
    }

    marks.verdict(Stub::BASE)
}

pub(crate) struct Marks {
    kept: Vec<String>,
    dropped: Vec<String>,
    factors: Vec<Factor>,
}

impl Marks {
    pub(crate) fn new() -> Self {
        Self {
            kept: Vec::new(),
            dropped: Vec::new(),
            factors: Vec::new(),
        }
    }

    pub(crate) fn keep(&mut self, item: impl Into<String>) {
        self.kept.push(item.into());
    }

    pub(crate) fn lose(&mut self, item: impl Into<String>) {
        self.dropped.push(item.into());
    }

    pub(crate) fn factor(&mut self, label: impl Into<String>, delta: i32, advice: &str) {
        self.factors.push(Factor {
            label: label.into(),
            delta,
            advice: advice.to_owned(),
        });
    }

    pub(crate) fn verdict(self, base: i32) -> Verdict {
        let score = aggregate(base, &self.factors);
        Verdict {
            summary: summarize(score, &self.factors),
            score,
            kept: self.kept,
            dropped: self.dropped,
            factors: self.factors,
        }
    }
}

/// A drop that was guessed from positioned text is not reported as a fact.
pub(crate) fn layout_label(input: &ParseInput, fact: &str) -> String {
    match input.layout_confidence {
        LayoutConfidence::Inferred => format!("{fact} (inferred)"),
        LayoutConfidence::Structural => fact.to_owned(),
    }
}

pub(crate) fn unreadable_verdict(base: i32) -> Verdict {
    let mut marks = Marks::new();
    marks.lose("file");
    marks.factor(
        "could not read file",
        -40,
        "Upload a PDF or DOCX the parser can open.",
    );
    marks.verdict(base)
}

fn read_pdf(bytes: &[u8]) -> ParseInput {
    let text = String::from_utf8_lossy(bytes);
    let mut parse = ParseInput::empty(LayoutConfidence::Inferred);
    parse.profile.email = first_email(&text);
    parse.layout.dates = date_style(&text);
    parse
}

fn read_docx(bytes: &[u8]) -> Option<ParseInput> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut body = String::new();
    let mut header = String::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).ok()?;
        let name = file.name().to_owned();
        let mut xml = String::new();
        file.read_to_string(&mut xml).ok()?;
        if name == "word/document.xml" {
            body = xml;
        } else if name.starts_with("word/header") || name.starts_with("word/footer") {
            header.push_str(&xml);
        }
    }
    if body.is_empty() {
        return None;
    }
    Some(parse_docx(&body, &header))
}

fn parse_docx(body: &str, header: &str) -> ParseInput {
    let body_texts = texts(body);
    let header_texts = texts(header);
    let body_text = body_texts.join(" ");
    let header_text = header_texts.join(" ");
    let mut parse = ParseInput::empty(LayoutConfidence::Structural);
    parse.profile.email = first_email(&body_text).or_else(|| first_email(&header_text));
    parse.profile.name = plausible_name(&header_texts).or_else(|| plausible_name(&body_texts));
    parse.profile.sections = headings(body);
    parse.layout.contact_in_header = first_email(&header_text).is_some();
    parse.layout.columns = column_count(body);
    parse.layout.has_tables = body.contains("<w:tbl");
    parse.layout.has_text_boxes = body.contains("<w:txbx");
    parse.layout.has_images = body.contains("<w:drawing") || body.contains("<a:blip");
    parse.layout.bullet_count = u32::try_from(body.matches("<w:numPr").count()).unwrap_or(u32::MAX);
    parse.layout.dates = date_style(&body_text);
    parse
}

fn column_count(body: &str) -> u32 {
    static COLS: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r#"<w:cols\b[^>]*\bw:num="(\d+)""#).expect("cols"));
    COLS.captures(body)
        .and_then(|cap| cap.get(1)?.as_str().parse().ok())
        .filter(|count| *count >= 1)
        .unwrap_or(1)
}

fn headings(body: &str) -> Vec<Section> {
    body.split("</w:p>")
        .filter(|paragraph| paragraph.contains("Heading"))
        .filter_map(|paragraph| {
            let heading = texts(paragraph).join(" ");
            let heading = heading.trim();
            if heading.is_empty() {
                None
            } else {
                Some(Section {
                    heading: heading.to_owned(),
                    body: String::new(),
                    in_table: false,
                    in_text_box: false,
                    in_header: false,
                })
            }
        })
        .collect()
}

fn texts(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<w:t") {
        let after = &rest[start..];
        let Some(open_end) = after.find('>') else {
            break;
        };
        let content = &after[open_end + 1..];
        let Some(close) = content.find("</w:t>") else {
            break;
        };
        let text = content[..close].replace("&amp;", "&");
        if !text.trim().is_empty() {
            out.push(text);
        }
        rest = &content[close + 6..];
    }
    out
}

fn plausible_name(values: &[String]) -> Option<String> {
    values.iter().map(|value| value.trim()).find_map(|value| {
        if value.contains(' ')
            && !value.contains('@')
            && value.len() < 60
            && value.chars().any(char::is_alphabetic)
        {
            Some(value.to_owned())
        } else {
            None
        }
    })
}

fn first_email(text: &str) -> Option<String> {
    static EMAIL: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("email")
    });
    EMAIL.find(text).map(|found| found.as_str().to_owned())
}

fn date_style(text: &str) -> DateStyle {
    static MONTH: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"\b(?:0?[1-9]|1[0-2])/(?:19|20)\d{2}\b").expect("month")
    });
    static YEAR: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\b(?:19|20)\d{2}\b").expect("year"));
    if MONTH.is_match(text) {
        DateStyle::MmYyyy
    } else if YEAR.is_match(text) {
        DateStyle::Other
    } else {
        DateStyle::Absent
    }
}

pub fn compiled(secret: String) -> Vec<Arc<dyn Ats>> {
    vec![Arc::new(Stub::new(secret))]
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use domain::{LayoutConfidence, ResumeFile};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::*;

    fn stored() -> SimpleFileOptions {
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
    }

    fn docx(header: &str, body: &str) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.start_file("word/header1.xml", stored()).unwrap();
        writer.write_all(header.as_bytes()).unwrap();
        writer.start_file("word/document.xml", stored()).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn designed() -> Vec<u8> {
        docx(
            "<w:t>Ada Lovelace</w:t><w:t>ada@example.com</w:t>",
            concat!(
                "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>Experience</w:t></w:r></w:p>",
                "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Rust</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
            ),
        )
    }

    fn plain_pdf() -> Vec<u8> {
        b"%PDF-1.4\n(Ada Lovelace) Tj (ada@example.com) Tj\n%%EOF\n".to_vec()
    }

    #[test]
    fn a_header_email_in_the_docx_scores_below_a_body_email_in_the_pdf() {
        let plugin = Stub::new("secret");
        let designed = plugin.read(&ResumeFile {
            media_type: domain::DOCX,
            bytes: &designed(),
        });
        let plain = plugin.read(&ResumeFile {
            media_type: domain::PDF,
            bytes: &plain_pdf(),
        });

        assert!(designed.parse.layout.contact_in_header);
        assert!(designed.parse.layout.has_tables);
        assert_eq!(
            designed.parse.layout_confidence,
            LayoutConfidence::Structural
        );
        assert_eq!(
            designed.parse.profile.email.as_deref(),
            Some("ada@example.com")
        );
        assert_eq!(designed.parse.profile.name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(designed.verdict.score, 15);
        assert_eq!(
            designed.verdict.summary,
            "Score 15 of 100. Move the email and phone into the body, not the header. Replace the table with plain lines of text. Keep the standard headings."
        );
        assert_eq!(
            designed.verdict.dropped,
            vec!["header contact".to_owned(), "table".to_owned()]
        );
        assert_eq!(designed.verdict.kept, vec!["Experience".to_owned()]);
        assert_eq!(
            designed.verdict.score,
            aggregate(Stub::BASE, &designed.verdict.factors)
        );
        assert!(
            designed
                .verdict
                .factors
                .iter()
                .all(|factor| factor.label != "prose")
        );

        assert!(!plain.parse.layout.contact_in_header);
        assert!(!plain.parse.layout.has_tables);
        assert_eq!(plain.parse.layout_confidence, LayoutConfidence::Inferred);
        assert_eq!(
            plain.parse.profile.email.as_deref(),
            Some("ada@example.com")
        );
        assert_eq!(plain.verdict.score, 65);
        assert_eq!(
            plain.verdict.summary,
            "Score 65 of 100. Keep the email in the body."
        );
        assert_eq!(plain.verdict.kept, vec!["email".to_owned()]);
        assert!(plain.verdict.dropped.is_empty());
        assert!(designed.verdict.score < plain.verdict.score);
    }

    #[test]
    fn an_unreadable_file_is_a_drop_not_a_guess() {
        let read = Stub::new("secret").read(&ResumeFile {
            media_type: domain::DOCX,
            bytes: b"not a zip",
        });
        assert_eq!(read.verdict.dropped, vec!["file".to_owned()]);
        assert!(read.parse.profile.email.is_none());
        assert_eq!(read.verdict.score, 10);
        assert_eq!(
            read.verdict.summary,
            "Score 10 of 100. Upload a PDF or DOCX the parser can open."
        );
    }
}
