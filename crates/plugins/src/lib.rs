//! Simulated ATS integrations. The spine still compiles in `stub` so an upload
//! can reach one callback without the Python providers. `fleet` is Greenline,
//! Levr, Workdaye, Ash, SmartResume, IndyHackersHiring, and BlastTrack. The running
//! API posts those files to the Python fake and does not call `read` or `call`.
//! `read` is what the fleet tests
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
mod blast_track;
mod greenline;
mod indy_hackers_hiring;
mod levr;
mod pdf;
mod smart_resume;
mod workdaye;

pub use ash::Ash;
pub use blast_track::BlastTrack;
pub use greenline::Greenline;
pub use indy_hackers_hiring::IndyHackersHiring;
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

/// Ordered text lines for the section detector. This is a separate read from
/// the frozen verification parse: it keeps paragraphs in document order
/// instead of reducing the file to a few fields.
pub fn lines(file: &ResumeFile<'_>) -> Vec<String> {
    document_lines(file)
        .into_iter()
        .map(|line| line.text)
        .collect()
}

pub fn document_lines(file: &ResumeFile<'_>) -> Vec<domain::DocumentLine> {
    match file.media_type {
        domain::DOCX => docx_lines(file.bytes),
        domain::PDF if file.bytes.starts_with(b"%PDF") => {
            if let Ok(lines) = pdf::extract_lines(file.bytes)
                && !lines.is_empty()
            {
                return lines;
            }
            raw_pdf_lines(file.bytes)
                .into_iter()
                .map(plain_line)
                .collect()
        }
        _ => split_lines(&String::from_utf8_lossy(file.bytes))
            .into_iter()
            .map(plain_line)
            .collect(),
    }
}

fn plain_line(text: String) -> domain::DocumentLine {
    domain::DocumentLine {
        text,
        ..Default::default()
    }
}

fn docx_lines(bytes: &[u8]) -> Vec<domain::DocumentLine> {
    let Ok(mut archive) = ZipArchive::new(Cursor::new(bytes)) else {
        return Vec::new();
    };
    let mut header = Vec::new();
    let mut body = Vec::new();
    let mut footer = Vec::new();
    for index in 0..archive.len() {
        let Ok(mut file) = archive.by_index(index) else {
            continue;
        };
        let name = file.name().to_owned();
        let mut xml = String::new();
        if file.read_to_string(&mut xml).is_err() {
            continue;
        }
        if name == "word/document.xml" {
            body.extend(paragraph_lines(&xml));
        } else if name.starts_with("word/header") {
            header.extend(paragraph_lines(&xml));
        } else if name.starts_with("word/footer") {
            footer.extend(paragraph_lines(&xml));
        }
    }
    header.extend(body);
    header.extend(footer);
    header
}

fn paragraph_lines(xml: &str) -> Vec<domain::DocumentLine> {
    xml.split("</w:p>")
        .filter_map(|paragraph| {
            // Runs split on formatting, not word boundaries, so concatenate
            // them. Whitespace lives inside the runs.
            let text = texts(paragraph).concat();
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() {
                return None;
            }
            static HEADING: LazyLock<regex::Regex> = LazyLock::new(|| {
                regex::Regex::new(r#"(?i)<w:pStyle\b[^>]*w:val=["']heading[1-9]["']"#).unwrap()
            });
            static BOLD: LazyLock<regex::Regex> = LazyLock::new(|| {
                regex::Regex::new(r#"<w:b\s*(?:/\s*>|w:val=["'](?:1|true|on)["']\s*/>)"#).unwrap()
            });
            static SIZE: LazyLock<regex::Regex> =
                LazyLock::new(|| regex::Regex::new(r#"<w:sz\b[^>]*w:val=["'](\d+)["']"#).unwrap());
            let runs: Vec<_> = paragraph
                .split("</w:r>")
                .filter(|run| !texts(run).concat().trim().is_empty())
                .collect();
            let bold = !runs.is_empty() && runs.iter().all(|run| BOLD.is_match(run));
            let font_size = SIZE
                .captures_iter(paragraph)
                .filter_map(|cap| cap[1].parse::<f32>().ok())
                .max_by(f32::total_cmp)
                .map(|size| size / 2.);
            Some(domain::DocumentLine {
                text,
                emphasized: HEADING.is_match(paragraph) || bold,
                font_size,
            })
        })
        .collect()
}

/// Last-resort read for a PDF the extractor rejects. Test fixtures and some
/// hand-built files have content streams with no page tree.
fn raw_pdf_lines(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::new();
    let mut rest = text.as_ref();
    while let Some(start) = rest.find('(') {
        let after = &rest[start + 1..];
        let Some(end) = after.find(')') else {
            break;
        };
        let literal = unescape_pdf(&after[..end]);
        let literal = literal.trim();
        if !literal.is_empty() {
            out.push(literal.to_owned());
        }
        rest = &after[end + 1..];
    }
    out
}

fn unescape_pdf(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(escaped) => out.push(escaped),
            None => break,
        }
    }
    out
}

fn split_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect()
}

/// The simulated fleet. The worker posts each file to the matching Python
/// provider instead of calling `read`.
pub fn fleet(secret: String) -> Vec<Arc<dyn Ats>> {
    vec![
        Arc::new(Greenline::new(secret.clone())),
        Arc::new(Levr::new(secret.clone())),
        Arc::new(Workdaye::new(secret.clone())),
        Arc::new(Ash::new(secret.clone())),
        Arc::new(SmartResume::new(secret.clone())),
        Arc::new(IndyHackersHiring::new(secret.clone())),
        Arc::new(BlastTrack::new(secret)),
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
            report: None,
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
    // These inputs may be paragraph/run fragments rather than complete XML.
    // Match only text elements, never similarly named table/text-box tags, and
    // never consume markup as text (including after a malformed text element).
    static TEXT: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"<w:t(?:\s+[^<>]*?[^/])?\s*>([^<]*)</w:t\s*>"#).unwrap()
    });
    TEXT.captures_iter(xml)
        .map(|capture| decode_xml_text(&capture[1]))
        .filter(|text| !text.trim().is_empty())
        .collect()
}

fn decode_xml_text(text: &str) -> String {
    static ENTITY: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"&(#x[0-9a-fA-F]+|#[0-9]+|amp|lt|gt|quot|apos);").unwrap()
    });
    ENTITY
        .replace_all(text, |capture: &regex::Captures<'_>| {
            let entity = &capture[1];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => entity
                    .strip_prefix("#x")
                    .and_then(|value| u32::from_str_radix(value, 16).ok())
                    .or_else(|| {
                        entity
                            .strip_prefix('#')
                            .and_then(|value| value.parse().ok())
                    })
                    .and_then(char::from_u32),
            };
            decoded
                .map(|value| value.to_string())
                .unwrap_or_else(|| capture[0].to_owned())
        })
        .into_owned()
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

    #[test]
    fn docx_retains_direct_formatting_hints_without_treating_disabled_bold_as_bold() {
        let lines = paragraph_lines(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Career</w:t></w:r></w:p><w:p><w:r><w:rPr><w:b/><w:sz w:val="32"/></w:rPr><w:t>Expertise</w:t></w:r></w:p><w:p><w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t>Normal</w:t></w:r></w:p>"#,
        );
        assert!(lines[0].emphasized);
        assert!(lines[1].emphasized);
        assert_eq!(lines[1].font_size, Some(16.));
        assert!(!lines[2].emphasized);
    }

    #[test]
    fn docx_lines_follow_document_order_and_join_split_runs() {
        let bytes = docx(
            "<w:p><w:r><w:t>Ada Lovelace</w:t></w:r></w:p><w:p><w:r><w:t>ada@example.com</w:t></w:r></w:p>",
            concat!(
                "<w:p><w:r><w:t>Ex</w:t></w:r><w:r><w:t>perience</w:t></w:r></w:p>",
                "<w:p><w:r><w:t>Staff Engineer</w:t></w:r></w:p>",
                "<w:p><w:r><w:t>   </w:t></w:r></w:p>",
                "<w:p><w:r><w:t>Education</w:t></w:r></w:p>",
            ),
        );
        let lines = lines(&ResumeFile {
            media_type: domain::DOCX,
            bytes: &bytes,
        });
        assert_eq!(
            lines,
            vec![
                "Ada Lovelace",
                "ada@example.com",
                "Experience",
                "Staff Engineer",
                "Education",
            ]
        );
    }

    #[test]
    fn docx_tables_and_text_boxes_extract_text_not_markup() {
        let body = concat!(
            "<w:tbl><w:tblPr/><w:tr><w:tc><w:tcPr/>",
            "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr>",
            "<w:r><w:t>Experience</w:t></w:r></w:p>",
            "<w:p><w:r><w:tab/><w:t xml:space=\"preserve\">Staff </w:t>",
            "<w:t>Engineer &amp; R&amp;D &lt;team&gt; &#65; &#x42;</w:t></w:r></w:p>",
            "</w:tc><w:tc><w:p><w:r><w:t>Second cell</w:t></w:r></w:p>",
            "</w:tc></w:tr></w:tbl>",
            "<w:p><w:r><w:txbxContent><w:p><w:r><w:t>Box text</w:t>",
            "</w:r></w:p></w:txbxContent></w:r></w:p>",
        );
        let bytes = docx("", body);
        assert_eq!(
            lines(&ResumeFile {
                media_type: domain::DOCX,
                bytes: &bytes
            }),
            vec![
                "Experience",
                "Staff Engineer & R&D <team> A B",
                "Second cell",
                "Box text"
            ]
        );
        assert_eq!(
            parse_docx(body, "").profile.sections[0].heading,
            "Experience"
        );
        assert_eq!(texts("<w:t/><w:tab/><w:t>ok</w:t>"), vec!["ok"]);
        assert_eq!(texts("<w:t>broken<w:tbl/><w:t>safe</w:t>"), vec!["safe"]);
        assert_eq!(
            texts("<w:t>&amp;lt; &quot; &apos;</w:t>"),
            vec!["&lt; \" '"]
        );
    }

    #[test]
    fn a_pdf_without_a_page_tree_falls_back_to_its_literals() {
        let bytes = plain_pdf();
        let lines = lines(&ResumeFile {
            media_type: domain::PDF,
            bytes: &bytes,
        });
        assert_eq!(lines, vec!["Ada Lovelace", "ada@example.com"]);
    }
}
