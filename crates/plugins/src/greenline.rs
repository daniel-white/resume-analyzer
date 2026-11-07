//! Standard headings and plain text. Tables, text boxes, and columns cost
//! points. The header and footer are ignored.

use domain::{Ats, ParseInput, ResumeFile, VendorRead};

use crate::{Marks, layout_label, unreadable_verdict};

pub struct Greenline {
    secret: String,
}

impl Greenline {
    pub const BASE: i32 = 50;

    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl Ats for Greenline {
    fn id(&self) -> &'static str {
        "greenline"
    }

    fn name(&self) -> &'static str {
        "Greenline"
    }

    fn subtitle(&self) -> &'static str {
        "Standard headings and plain text. Ignores the header. Simulated, modeled on publicly described parser behavior."
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

    // Header and footer contact is not read. The factor says so.
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
        marks.factor(
            layout_label(input, "table"),
            -20,
            "Replace the table with plain lines of text.",
        );
    }
    if input.layout.has_text_boxes {
        marks.lose("text box");
        marks.factor(
            layout_label(input, "text box"),
            -15,
            "Take the text out of the text box.",
        );
    }
    if input.layout.columns >= 2 {
        marks.lose("columns");
        marks.factor(
            layout_label(input, "two columns"),
            -15,
            "Keep the page to one column.",
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

    marks.verdict(Greenline::BASE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::LayoutConfidence;

    #[test]
    fn a_pdf_table_is_inferred_not_a_fact() {
        let mut input = ParseInput::empty(LayoutConfidence::Inferred);
        input.layout.has_tables = true;
        let verdict = decide(&input);
        assert!(
            verdict
                .factors
                .iter()
                .any(|factor| { factor.label == "table (inferred)" && factor.delta < 0 })
        );
        assert!(verdict.factors.iter().all(|factor| factor.label != "table"));
    }
}
