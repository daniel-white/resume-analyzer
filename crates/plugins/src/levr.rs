//! Section-agnostic. Skills count only as exact tokens, and bullet density helps.

use domain::{Ats, ParseInput, ResumeFile, VendorRead};

use crate::{Marks, unreadable_verdict};

pub struct Levr {
    secret: String,
}

impl Levr {
    pub const BASE: i32 = 70;

    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl Ats for Levr {
    fn id(&self) -> &'static str {
        "levr"
    }

    fn name(&self) -> &'static str {
        "Levr"
    }

    fn subtitle(&self) -> &'static str {
        "Exact skill tokens and bullet density. Simulated, modeled on publicly described parser behavior."
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

    // A skills table is not an exact token. Headings are not a signal.
    if input.layout.has_tables {
        marks.lose("skills");
        marks.factor(
            "skills not normalized",
            -15,
            "Write skills as exact tokens on their own lines.",
        );
    }
    if input.layout.bullet_count >= 2 {
        marks.keep("experience bullets");
        marks.factor(
            "bullet density",
            20,
            "Keep the bullets. This parser counts them.",
        );
    }

    marks.verdict(Levr::BASE)
}
