//! Drops tables, headers, text boxes, and multi-column layout. Contact has to
//! sit in the body, and dates have to be MM/YYYY.

use domain::{Ats, DateStyle, ParseInput, ResumeFile, VendorRead};

use crate::{Marks, layout_label, unreadable_verdict};

pub struct Workdaye {
    secret: String,
}

impl Workdaye {
    pub const BASE: i32 = 70;

    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl Ats for Workdaye {
    fn id(&self) -> &'static str {
        "workdaye"
    }

    fn name(&self) -> &'static str {
        "Workdaye"
    }

    fn subtitle(&self) -> &'static str {
        "Contact in the body, dates as MM/YYYY. Simulated, modeled on publicly described parser behavior."
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

    match input.layout.dates {
        DateStyle::MmYyyy => {
            marks.factor("dates MM/YYYY", 10, "Keep dates in MM/YYYY.");
        }
        DateStyle::Other | DateStyle::Absent => {
            marks.lose("dates");
            marks.factor("dates not MM/YYYY", -12, "Write dates as MM/YYYY.");
        }
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

    marks.verdict(Workdaye::BASE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::LayoutConfidence;

    #[test]
    fn a_pdf_table_is_inferred_not_a_fact() {
        let mut input = ParseInput::empty(LayoutConfidence::Inferred);
        input.layout.has_tables = true;
        input.layout.dates = DateStyle::MmYyyy;
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
