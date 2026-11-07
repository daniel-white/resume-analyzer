//! Keeps table text and header contact, but loses text-box content.

use domain::{Ats, ParseInput, ResumeFile, VendorRead};

use crate::{Marks, layout_label, unreadable_verdict};

pub struct SmartResume {
    secret: String,
}

impl SmartResume {
    pub const BASE: i32 = 50;

    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl Ats for SmartResume {
    fn id(&self) -> &'static str {
        "smart_resume"
    }

    fn name(&self) -> &'static str {
        "SmartResume"
    }

    fn subtitle(&self) -> &'static str {
        "Keeps table text and headers, loses text boxes. Simulated, modeled on publicly described parser behavior."
    }

    fn secret(&self) -> &str {
        &self.secret
    }

    fn read(&self, file: &ResumeFile<'_>) -> VendorRead {
        let (parse, unreadable_file) = crate::extract(file);
        let verdict = if unreadable_file {
            unreadable_verdict(Self::BASE)
        } else {
            decide(&parse)
        };
        VendorRead { parse, verdict }
    }
}

fn decide(input: &ParseInput) -> domain::Verdict {
    let mut marks = Marks::new();

    if input.profile.email.is_some() {
        marks.keep("email");
        marks.factor("email kept", 15, "Keep the email where it is.");
    }
    if input.layout.has_tables {
        marks.keep("table text");
        marks.factor(
            layout_label(input, "table text kept"),
            10,
            "Keep the table text. This parser reads its cells.",
        );
    }
    if input.layout.has_text_boxes {
        marks.lose("text box");
        marks.factor(
            layout_label(input, "text box"),
            -20,
            "Take the text out of the text box.",
        );
    }
    if input
        .profile
        .sections
        .iter()
        .any(|section| !section.heading.is_empty())
    {
        marks.keep("headings");
        marks.factor("standard headings", 10, "Keep the standard headings.");
    }
    if input.layout.bullet_count >= 2 {
        marks.keep("experience bullets");
        marks.factor(
            "bullet density",
            15,
            "Keep the bullets. This parser counts them.",
        );
    }

    marks.verdict(SmartResume::BASE)
}

#[cfg(test)]
mod tests {
    use domain::LayoutConfidence;

    use super::*;

    #[test]
    fn inferred_layout_is_not_reported_as_fact() {
        let mut parse = ParseInput::empty(LayoutConfidence::Inferred);
        parse.layout.has_tables = true;
        parse.layout.has_text_boxes = true;
        let verdict = decide(&parse);
        assert_eq!(verdict.factors[0].label, "table text kept (inferred)");
        assert!(verdict.factors[0].delta > 0);
        assert_eq!(verdict.factors[1].label, "text box (inferred)");
        assert!(verdict.factors[1].delta < 0);
    }

    #[test]
    fn unreadable_file_is_a_drop_not_a_guess() {
        let read = SmartResume::new("secret").read(&ResumeFile {
            media_type: domain::DOCX,
            bytes: b"not a zip",
        });
        assert_eq!(read.verdict.dropped, ["file"]);
        assert_eq!(read.verdict.factors[0].label, "could not read file");
        assert!(read.verdict.factors[0].delta < 0);
        assert!(read.parse.profile.email.is_none());
    }
}
