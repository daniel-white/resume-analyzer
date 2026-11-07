//! Keeps more of the structure, normalizes a skills table, and tolerates two
//! columns. Images are still dropped.

use domain::{Ats, ParseInput, ResumeFile, VendorRead};

use crate::{Marks, unreadable_verdict};

pub struct Ash {
    secret: String,
}

impl Ash {
    pub const BASE: i32 = 75;

    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl Ats for Ash {
    fn id(&self) -> &'static str {
        "ash"
    }

    fn name(&self) -> &'static str {
        "Ash"
    }

    fn subtitle(&self) -> &'static str {
        "Keeps more structure, still drops images. Simulated, modeled on publicly described parser behavior."
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
        marks.factor("email kept", 10, "Keep the email where it is.");
    }
    // Two columns are tolerated: no column factor. A skills table is kept.
    if input.layout.has_tables {
        marks.keep("skills");
        marks.factor(
            "skills normalized",
            8,
            "Keep the skills list. This parser normalizes it.",
        );
    }
    if input.layout.has_images {
        marks.lose("image");
        marks.factor("image", -10, "Leave images off the page.");
    }
    if input
        .profile
        .sections
        .iter()
        .any(|section| !section.heading.is_empty())
    {
        marks.keep("headings");
        marks.factor("standard headings", 5, "Keep the standard headings.");
    }

    marks.verdict(Ash::BASE)
}
