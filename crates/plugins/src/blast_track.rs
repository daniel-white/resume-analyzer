//! Preserves contact and table text, but requires linear reading order and clear dates.

use domain::{Ats, DateStyle, ParseInput, ResumeFile, VendorRead};

use crate::{Marks, layout_label, unreadable_verdict};

pub struct BlastTrack {
    secret: String,
}

impl BlastTrack {
    pub const BASE: i32 = 70;

    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl Ats for BlastTrack {
    fn id(&self) -> &'static str {
        "blast_track"
    }

    fn name(&self) -> &'static str {
        "BlastTrack"
    }

    fn subtitle(&self) -> &'static str {
        "Keeps contact and table text, needs linear chronology. Simulated, modeled on publicly described parser behavior."
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
    // Table and text-box text survive; only competing columns disrupt ordering.
    if input.layout.has_tables {
        marks.keep("table text");
    }
    if input.layout.has_text_boxes {
        marks.keep("text box text");
    }
    if input.layout.columns >= 2 {
        marks.lose("column reading order");
        marks.factor(
            layout_label(input, "column reading order"),
            -18,
            "Put experience in one column to preserve reading order.",
        );
    }
    match input.layout.dates {
        DateStyle::Other => {
            marks.lose("date precision");
            marks.factor(
                "ambiguous chronology",
                -12,
                "Write dates as MM/YYYY to make the chronology unambiguous.",
            );
        }
        DateStyle::MmYyyy => {
            marks.keep("dates");
            marks.factor("dates MM/YYYY", 10, "Keep dates in MM/YYYY.");
        }
        DateStyle::Absent => {}
    }
    marks.verdict(BlastTrack::BASE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::LayoutConfidence;

    #[test]
    fn inferred_reading_order_is_not_a_structural_fact() {
        let mut input = ParseInput::empty(LayoutConfidence::Inferred);
        input.layout.columns = 2;
        let verdict = decide(&input);
        assert_eq!(verdict.factors[0].label, "column reading order (inferred)");
        assert!(verdict.factors[0].delta < 0);
    }
}
