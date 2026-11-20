//! Build a per-version writing checklist from the stored reading.
//!
//! This generator does not call a model. It combines stored sections, skills
//! and layout flags with the bad-practice flags the model already returned and
//! Rust validated. Every quote or deletion is copied verbatim from a detected
//! section. A blank is the writer's own line with a hole left in it, so the app
//! can never put a fact into the résumé that the writer did not supply.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, ensure};
use domain::{ChecklistKind, ParseInput, SectionKind, SectionMap, Skill, SkillSet, WritingIssue};
use sha2::{Digest, Sha256};

const MAX_BLANKS: usize = 4;
const MAX_PROMOTES: usize = 2;
const MAX_CUTS: usize = 2;
const MAX_FLAGS: usize = 4;
const MAX_ITEMS: usize = MAX_BLANKS + MAX_PROMOTES + MAX_CUTS + MAX_FLAGS;
const QUOTE_CHARS: usize = 240;
const TITLE_CHARS: usize = 90;

/// One generated item before storage adds an id, ordering and the reader's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedItem {
    pub fingerprint: String,
    pub kind: ChecklistKind,
    pub title: String,
    pub detail: String,
    pub why: String,
    pub placeholder: String,
    pub section_id: Option<String>,
    pub section_heading: Option<String>,
    pub quote: Option<String>,
    pub template: Option<String>,
    pub cut_text: Option<String>,
    pub position: i32,
}

/// Build the checklist in reading order: blanks, promoted proof, cuts, then
/// model-flagged bad practices. The result is deterministic, so regeneration
/// keeps the same fingerprints and therefore the reader's checked state.
#[must_use]
pub fn generate(
    sections: &SectionMap,
    skills: &SkillSet,
    parse: &ParseInput,
    issues: &[WritingIssue],
) -> Vec<GeneratedItem> {
    let blanks = blanks(sections, skills);
    let blanked = blanks
        .iter()
        .filter_map(|item| item.fingerprint.strip_prefix("blank:").map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let mut items = blanks;
    items.extend(promotes(sections, skills, parse, &blanked));
    items.extend(cuts(sections, skills));
    items.extend(flags(sections, issues));
    for (index, item) in items.iter_mut().enumerate() {
        item.position = i32::try_from(index).unwrap_or(i32::MAX);
    }
    items
}

/// Refuse to store anything that cannot be traced back to the document. The
/// generator grounds items by construction; this is the guard for future edits.
pub fn validate(items: &[GeneratedItem], sections: &SectionMap) -> Result<()> {
    ensure!(items.len() <= MAX_ITEMS, "too many checklist items");
    let mut seen = BTreeSet::new();
    for item in items {
        ensure!(
            seen.insert(item.fingerprint.as_str()),
            "duplicate checklist item"
        );
        ensure!(!item.title.trim().is_empty(), "empty checklist title");
        ensure!(
            !item.placeholder.trim().is_empty(),
            "empty checklist placeholder"
        );
        if item.quote.is_some() || item.cut_text.is_some() {
            let id = item
                .section_id
                .as_deref()
                .ok_or_else(|| anyhow!("item has no source section"))?;
            let section = sections
                .sections
                .iter()
                .find(|section| section.id == id)
                .ok_or_else(|| anyhow!("item cites an unknown section"))?;
            let body = normalize(&section.body);
            if let Some(quote) = &item.quote {
                ensure!(
                    body.contains(&normalize(quote)),
                    "quote is not in the cited section"
                );
            }
            if let Some(cut) = &item.cut_text {
                ensure!(
                    body.contains(&normalize(cut)),
                    "cut text is not in the cited section"
                );
            }
        }
    }
    Ok(())
}

fn blanks(sections: &SectionMap, skills: &SkillSet) -> Vec<GeneratedItem> {
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    for skill in &skills.skills {
        if items.len() >= MAX_BLANKS || !seen.insert(skill.canonical.as_str()) {
            continue;
        }
        let Some(evidence) = skill.evidence.iter().find(|item| needs_number(&item.quote)) else {
            continue;
        };
        let Some(section) = find_section(sections, &evidence.section_id) else {
            continue;
        };
        let quote = clip(evidence.quote.trim(), QUOTE_CHARS);
        let template = format!("{} — ___", quote.trim_end_matches(['.', ';', ',']).trim());
        items.push(GeneratedItem {
            fingerprint: hash(
                "blank",
                &format!(
                    "{}|{}|{}",
                    skill.canonical,
                    evidence.section_id,
                    normalize(&quote),
                ),
            ),
            kind: ChecklistKind::Blank,
            title: clip(&format!("Add a number to {}", skill.name), TITLE_CHARS),
            detail: format!("Your line: \"{quote}\""),
            why: reason(skill),
            placeholder: format!(
                "how many {} · how often · what changed",
                skill.name.to_lowercase()
            ),
            section_id: Some(evidence.section_id.clone()),
            section_heading: Some(section.heading.clone()),
            quote: Some(quote),
            template: Some(template),
            cut_text: None,
            position: 0,
        });
    }
    if items.is_empty()
        && let Some(item) = section_blank(sections)
    {
        items.push(item);
    }
    items
}

/// When the model found no skill-level gap, a duty-only newest role is still
/// worth one blank. The quote stays the writer's own text.
fn section_blank(sections: &SectionMap) -> Option<GeneratedItem> {
    let section = sections
        .sections
        .iter()
        .find(|item| item.kind == SectionKind::Experience && item.body.len() >= 40)?;
    let line =
        section.body.lines().map(str::trim).find(|line| {
            (12..=120).contains(&line.split_whitespace().count()) && !has_digit(line)
        })?;
    let quote = clip(line, QUOTE_CHARS);
    let template = format!("{} — ___", quote.trim_end_matches(['.', ';', ',']).trim());
    Some(GeneratedItem {
        fingerprint: hash(
            "blank",
            &format!("section|{}|{}", section.id, normalize(&quote)),
        ),
        kind: ChecklistKind::Blank,
        title: "Add numbers to your newest role".to_owned(),
        detail: format!("Your line: \"{quote}\""),
        why: "This role is described without a single number, so a reader cannot judge its scale."
            .to_owned(),
        placeholder: "how many · how often · what changed".to_owned(),
        section_id: Some(section.id.clone()),
        section_heading: Some(section.heading.clone()),
        quote: Some(quote),
        template: Some(template),
        cut_text: None,
        position: 0,
    })
}

fn promotes(
    sections: &SectionMap,
    skills: &SkillSet,
    parse: &ParseInput,
    blanked: &BTreeSet<String>,
) -> Vec<GeneratedItem> {
    let mut items = Vec::new();
    for skill in &skills.skills {
        if items.len() >= MAX_PROMOTES {
            break;
        }
        if skill.importance < 60 || blanked.contains(skill.canonical.as_str()) {
            continue;
        }
        let Some(evidence) = skill.evidence.iter().max_by_key(|item| item.quote.len()) else {
            continue;
        };
        let Some((index, section)) = sections
            .sections
            .iter()
            .enumerate()
            .find(|(_, section)| section.id == evidence.section_id)
        else {
            continue;
        };
        let list_only = matches!(
            section.kind,
            SectionKind::Skills | SectionKind::Interests | SectionKind::Languages
        );
        let hidden_in_layout = parse.profile.sections.iter().any(|item| {
            normalize(&item.heading) == normalize(&section.heading)
                && (item.in_table || item.in_text_box || item.in_header)
        });
        if !list_only && !hidden_in_layout && index < 2 {
            continue;
        }
        let quote = clip(evidence.quote.trim(), QUOTE_CHARS);
        let template = format!("{} — ___", quote.trim_end_matches(['.', ';', ',']).trim());
        items.push(GeneratedItem {
            fingerprint: hash("promote", &format!("{}|{}", skill.canonical, evidence.section_id)),
            kind: ChecklistKind::Promote,
            title: clip(&format!("Bring {} up top", skill.name), TITLE_CHARS),
            detail: format!(
                "Your best proof for {} — \"{quote}\" — sits in {}.",
                skill.name, section.heading
            ),
            why: "This is one of the most substantiated claims you have. Put it where a reader meets it first."
                .to_owned(),
            placeholder: format!(
                "move it to your summary or newest role · add a number for {}",
                skill.name.to_lowercase()
            ),
            section_id: Some(section.id.clone()),
            section_heading: Some(section.heading.clone()),
            quote: Some(quote),
            template: Some(template),
            cut_text: None,
            position: 0,
        });
    }
    items
}

/// Generic traits and duty phrases with no number attached. Each hit must be a
/// whole line copied from a section, never a paraphrase.
const FLUFF: [&str; 14] = [
    "hard working",
    "hard-working",
    "team player",
    "passion for",
    "passionate about",
    "results driven",
    "results-driven",
    "detail oriented",
    "detail-oriented",
    "self starter",
    "self-starter",
    "go getter",
    "go-getter",
    "references available upon request",
];

fn cuts(sections: &SectionMap, skills: &SkillSet) -> Vec<GeneratedItem> {
    let evidence = skills
        .skills
        .iter()
        .flat_map(|skill| skill.evidence.iter().map(|item| normalize(&item.quote)))
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    for section in &sections.sections {
        if items.len() >= MAX_CUTS {
            break;
        }
        for line in section.body.lines().map(str::trim) {
            if items.len() >= MAX_CUTS || !is_fluff(line, section, &evidence) {
                continue;
            }
            if !seen.insert(normalize(line)) {
                continue;
            }
            let text = clip(line, QUOTE_CHARS);
            items.push(GeneratedItem {
                fingerprint: hash("cut", &format!("{}|{}", section.id, normalize(line))),
                kind: ChecklistKind::Cut,
                title: clip(&format!("Cut: \"{text}\""), TITLE_CHARS),
                detail: format!("Found in {}: \"{text}\"", section.heading),
                why: "An unsupported trait or duty. It costs space without showing a result."
                    .to_owned(),
                placeholder: "what result will you put in its place?".to_owned(),
                section_id: Some(section.id.clone()),
                section_heading: Some(section.heading.clone()),
                quote: None,
                template: None,
                cut_text: Some(text),
                position: 0,
            });
        }
    }
    items
}

fn is_fluff(line: &str, section: &domain::DetectedSection, evidence: &[String]) -> bool {
    if line.len() < 12 || line.len() > 180 || has_digit(line) {
        return false;
    }
    if normalize(line) == normalize(&section.heading) {
        return false;
    }
    let lowered = line.to_ascii_lowercase();
    if !FLUFF.iter().any(|phrase| lowered.contains(phrase)) {
        return false;
    }
    let text = normalize(line);
    !evidence.iter().any(|quote| text.contains(quote.as_str()))
}

/// Turn each validated model flag into a grounded edit. The generator checks
/// the quote again, so stale or hand-edited stored flags cannot surface text
/// that is no longer in the detected section.
fn flags(sections: &SectionMap, issues: &[WritingIssue]) -> Vec<GeneratedItem> {
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    for issue in issues {
        if items.len() >= MAX_FLAGS {
            break;
        }
        let Some(section) = find_section(sections, &issue.section_id) else {
            continue;
        };
        let quote = issue.quote.trim();
        if quote.is_empty() || !normalize(&section.body).contains(&normalize(quote)) {
            continue;
        }
        if !seen.insert((
            normalize(&issue.label),
            section.id.clone(),
            normalize(quote),
        )) {
            continue;
        }
        let text = clip(quote, QUOTE_CHARS);
        items.push(GeneratedItem {
            fingerprint: hash(
                "flag",
                &format!(
                    "{}|{}|{}",
                    normalize(&issue.label),
                    section.id,
                    normalize(quote)
                ),
            ),
            kind: ChecklistKind::Flag,
            title: clip(&format!("Flag: {}", issue.label), TITLE_CHARS),
            detail: format!("Found in {}: \"{text}\"", section.heading),
            why: issue.why.clone(),
            placeholder: issue.fix.clone(),
            section_id: Some(section.id.clone()),
            section_heading: Some(section.heading.clone()),
            quote: Some(text),
            template: None,
            cut_text: None,
            position: 0,
        });
    }
    items
}

fn reason(skill: &Skill) -> String {
    let text = skill.confidence_reason.trim();
    if (20..=180).contains(&text.len()) {
        let mut chars = text.chars();
        match chars.next() {
            Some(first) => format!(
                "{}{}.",
                first.to_uppercase(),
                chars.as_str().trim_end_matches('.')
            ),
            None => generic_reason(skill),
        }
    } else {
        generic_reason(skill)
    }
}

fn generic_reason(skill: &Skill) -> String {
    format!(
        "{} is one of your stronger signals, but this line is a duty rather than a result.",
        skill.name
    )
}

fn needs_number(quote: &str) -> bool {
    let text = quote.trim();
    let words = text.split_whitespace().count();
    !text.is_empty() && (12..=40).contains(&words) && !has_digit(text)
}

fn has_digit(text: &str) -> bool {
    text.chars().any(|ch| ch.is_ascii_digit())
}

fn find_section<'a>(sections: &'a SectionMap, id: &str) -> Option<&'a domain::DetectedSection> {
    sections.sections.iter().find(|section| section.id == id)
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn hash(kind: &str, key: &str) -> String {
    let digest = Sha256::digest(format!("{kind}|{key}").as_bytes());
    hex::encode(digest)
}

fn clip(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", text[..end].trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        DetectedSection, Layout, LayoutConfidence, Profile, SectionVote, SkillCategory,
        SkillEvidence, SkillNature,
    };

    fn section(id: &str, kind: SectionKind, heading: &str, body: &str) -> DetectedSection {
        DetectedSection {
            id: id.to_owned(),
            kind,
            heading: heading.to_owned(),
            body: body.to_owned(),
            confidence: 0.9,
            feedback: Option::<SectionVote>::None,
        }
    }

    fn skill(name: &str, quote: &str, section_id: &str, importance: u8) -> Skill {
        Skill {
            name: name.to_owned(),
            canonical: name.to_ascii_lowercase(),
            category: SkillCategory::Method,
            nature: SkillNature::Hard,
            importance,
            confidence: 70,
            confidence_reason: String::new(),
            evidence: vec![SkillEvidence {
                section_id: section_id.to_owned(),
                quote: quote.to_owned(),
            }],
        }
    }

    fn parse(sections: Vec<domain::Section>) -> ParseInput {
        ParseInput {
            profile: Profile {
                name: None,
                email: None,
                phone: None,
                sections,
            },
            layout: Layout {
                columns: 1,
                contact_in_header: false,
                has_tables: false,
                has_text_boxes: false,
                has_images: false,
                bullet_count: 0,
                dates: domain::DateStyle::MmYyyy,
            },
            layout_confidence: LayoutConfidence::Structural,
        }
    }

    fn layout_section(heading: &str, table: bool) -> domain::Section {
        domain::Section {
            heading: heading.to_owned(),
            body: String::new(),
            in_table: table,
            in_text_box: false,
            in_header: false,
        }
    }

    #[test]
    fn blanks_use_the_writers_own_line_and_leave_a_hole() {
        let quote = "Triaged patients arriving in the busy emergency department during long weekend shifts.";
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "test".to_owned(),
            sections: vec![section(
                "experience-1",
                SectionKind::Experience,
                "Experience",
                quote,
            )],
        };
        let skills = SkillSet {
            skills: vec![skill("Triage", quote, "experience-1", 80)],
            engine: "chat".to_owned(),
            model: "test".to_owned(),
        };
        let items = generate(&sections, &skills, &parse(Vec::new()), &[]);
        validate(&items, &sections).expect("items are grounded");
        let blank = items
            .iter()
            .find(|item| item.kind == ChecklistKind::Blank)
            .expect("a blank");
        assert_eq!(blank.quote.as_deref(), Some(quote));
        assert!(
            blank
                .template
                .as_deref()
                .is_some_and(|text| text.ends_with("— ___"))
        );
        assert!(blank.placeholder.contains("triage"));
    }

    #[test]
    fn a_number_in_the_line_prevents_a_blank() {
        let quote = "Triaged 40 patients per shift in the emergency department.";
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "test".to_owned(),
            sections: vec![section(
                "experience-1",
                SectionKind::Experience,
                "Experience",
                quote,
            )],
        };
        let skills = SkillSet {
            skills: vec![skill("Triage", quote, "experience-1", 80)],
            engine: "chat".to_owned(),
            model: "test".to_owned(),
        };
        let items = generate(&sections, &skills, &parse(Vec::new()), &[]);
        assert!(items.iter().all(|item| item.kind != ChecklistKind::Blank));
    }

    #[test]
    fn buried_evidence_is_promoted() {
        let quote = "Resolved more than two hundred escalations every quarter for key accounts.";
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "test".to_owned(),
            sections: vec![
                section(
                    "summary-1",
                    SectionKind::Summary,
                    "Summary",
                    "A short summary.",
                ),
                section("skills-1", SectionKind::Skills, "Skills", quote),
            ],
        };
        let skills = SkillSet {
            skills: vec![skill("Escalations", quote, "skills-1", 90)],
            engine: "chat".to_owned(),
            model: "test".to_owned(),
        };
        let items = generate(
            &sections,
            &skills,
            &parse(vec![layout_section("Skills", false)]),
            &[],
        );
        validate(&items, &sections).expect("items are grounded");
        assert!(items.iter().any(|item| item.kind == ChecklistKind::Promote));
    }

    #[test]
    fn fluff_is_cut_but_a_result_is_not() {
        let body = "Hard-working team player with a passion for excellence.\nDelivered 12 projects on time.";
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "test".to_owned(),
            sections: vec![section("summary-1", SectionKind::Summary, "Summary", body)],
        };
        let skills = SkillSet {
            skills: Vec::new(),
            engine: "chat".to_owned(),
            model: "test".to_owned(),
        };
        let items = generate(&sections, &skills, &parse(Vec::new()), &[]);
        validate(&items, &sections).expect("items are grounded");
        let cut = items
            .iter()
            .find(|item| item.kind == ChecklistKind::Cut)
            .expect("a cut");
        assert_eq!(
            cut.cut_text.as_deref(),
            Some("Hard-working team player with a passion for excellence.")
        );
    }

    #[test]
    fn regeneration_keeps_fingerprints_stable() {
        let quote = "Triaged patients arriving in the busy emergency department during long weekend shifts.";
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "test".to_owned(),
            sections: vec![section(
                "experience-1",
                SectionKind::Experience,
                "Experience",
                quote,
            )],
        };
        let skills = SkillSet {
            skills: vec![skill("Triage", quote, "experience-1", 80)],
            engine: "chat".to_owned(),
            model: "test".to_owned(),
        };
        let first = generate(&sections, &skills, &parse(Vec::new()), &[]);
        let second = generate(&sections, &skills, &parse(Vec::new()), &[]);
        assert_eq!(first, second);
    }

    fn writing_issue(label: &str, quote: &str) -> WritingIssue {
        WritingIssue {
            label: label.to_owned(),
            why: "It shifts the focus from your work to a person.".to_owned(),
            fix: "Describe the gap without naming anyone.".to_owned(),
            section_id: "experience-1".to_owned(),
            quote: quote.to_owned(),
        }
    }

    #[test]
    fn model_flags_become_grounded_checklist_items() {
        let quote = "My old manager never explained the process.";
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "test".to_owned(),
            sections: vec![section(
                "experience-1",
                SectionKind::Experience,
                "Experience",
                quote,
            )],
        };
        let skills = SkillSet {
            skills: Vec::new(),
            engine: "chat".to_owned(),
            model: "test".to_owned(),
        };
        let items = generate(
            &sections,
            &skills,
            &parse(Vec::new()),
            &[writing_issue("Blaming a former manager", quote)],
        );
        validate(&items, &sections).expect("items are grounded");
        let flag = items
            .iter()
            .find(|item| item.kind == ChecklistKind::Flag)
            .expect("a flag");
        assert_eq!(flag.quote.as_deref(), Some(quote));
        assert_eq!(flag.placeholder, "Describe the gap without naming anyone.");
        assert!(flag.title.contains("Blaming a former manager"));
    }

    #[test]
    fn an_ungrounded_flag_cannot_reach_the_checklist() {
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "test".to_owned(),
            sections: vec![section(
                "experience-1",
                SectionKind::Experience,
                "Experience",
                "Real text.",
            )],
        };
        let skills = SkillSet {
            skills: Vec::new(),
            engine: "chat".to_owned(),
            model: "test".to_owned(),
        };
        let items = generate(
            &sections,
            &skills,
            &parse(Vec::new()),
            &[writing_issue("Name-calling", "A line never written.")],
        );
        assert_eq!(items, [] as [GeneratedItem; 0]);
    }

    #[test]
    fn validation_rejects_an_ungrounded_quote() {
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "test".to_owned(),
            sections: vec![section(
                "experience-1",
                SectionKind::Experience,
                "Experience",
                "Real text.",
            )],
        };
        let item = GeneratedItem {
            fingerprint: "x".to_owned(),
            kind: ChecklistKind::Blank,
            title: "A title".to_owned(),
            detail: "A detail".to_owned(),
            why: "A reason".to_owned(),
            placeholder: "A hint".to_owned(),
            section_id: Some("experience-1".to_owned()),
            section_heading: Some("Experience".to_owned()),
            quote: Some("Invented text that was never written.".to_owned()),
            template: None,
            cut_text: None,
            position: 0,
        };
        assert!(validate(std::slice::from_ref(&item), &sections).is_err());
    }
}
