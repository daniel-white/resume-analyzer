//! Compact intake: tolerates tables and text boxes, loses columns and images.

use domain::{Ats, ParseInput, ResumeFile, VendorRead};

use crate::{Marks, layout_label, unreadable_verdict};

pub struct IndyHackersHiring {
    secret: String,
}

impl IndyHackersHiring {
    pub const BASE: i32 = 70;

    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl Ats for IndyHackersHiring {
    fn id(&self) -> &'static str {
        "indy_hackers_hiring"
    }
    fn name(&self) -> &'static str {
        "IndyHackersHiring"
    }
    fn subtitle(&self) -> &'static str {
        "Tolerates tables and text boxes, loses columns and images. Simulated, modeled on publicly described parser behavior."
    }
    fn secret(&self) -> &str {
        &self.secret
    }
    fn read(&self, file: &ResumeFile<'_>) -> VendorRead {
        let (parse, unreadable_file) = crate::extract(file);
        let verdict = if unreadable_file {
            unreadable_verdict(Self::BASE - 20)
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
    if input.layout.columns >= 2 {
        marks.lose("columns");
        marks.factor(
            layout_label(input, "two columns"),
            -18,
            "Keep the page to one column.",
        );
    }
    if input.layout.has_images {
        marks.lose("image");
        marks.factor("image", -12, "Leave images off the page.");
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
    marks.verdict(IndyHackersHiring::BASE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::LayoutConfidence;

    #[test]
    fn inferred_columns_are_not_reported_as_fact_and_tables_are_tolerated() {
        let mut parse = ParseInput::empty(LayoutConfidence::Inferred);
        parse.layout.columns = 2;
        parse.layout.has_tables = true;
        parse.layout.has_text_boxes = true;
        let verdict = decide(&parse);
        assert_eq!(verdict.factors.len(), 1);
        assert_eq!(verdict.factors[0].label, "two columns (inferred)");
        assert!(verdict.factors[0].delta < 0);
        assert_eq!(verdict.dropped, ["columns"]);
    }

    #[test]
    fn unreadable_file_has_no_readable_uplift() {
        let read = IndyHackersHiring::new("secret").read(&ResumeFile {
            media_type: domain::DOCX,
            bytes: b"not a zip",
        });
        assert_eq!(read.verdict.dropped, ["file"]);
        assert_eq!(read.verdict.factors[0].label, "could not read file");
        assert!(read.verdict.factors[0].delta < 0);
        assert_eq!(read.verdict.score, 10);
    }
}
