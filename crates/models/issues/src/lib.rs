//! Bad-practice flags from the one combined résumé read.
//!
//! The model names the practice, explains the cost and suggests a fix, but it
//! must also quote the line it is flagging. Rust drops any flag whose quote is
//! not found verbatim in the cited stored section. Nothing invented here can
//! reach the writer's checklist.

use std::collections::BTreeSet;

use domain::{SectionMap, WritingIssue, WritingIssueSet};
use serde::{Deserialize, Serialize};
use stream_protocol::{ModelEvent, StreamOutput};

/// Keep the Second Draft short enough to finish.
const MAX_ISSUES: usize = 4;
const MAX_LABEL_CHARS: usize = 80;
const MAX_WHY_CHARS: usize = 180;
const MAX_FIX_CHARS: usize = 160;
const MAX_QUOTE_CHARS: usize = 240;

/// Known bad practices the model should look for. The list is a prompt, not a
/// whitelist: a clear problem outside it is still worth one grounded flag.
pub const ISSUE_RULES: &str = "Also flag up to four bad résumé practices you can prove from the \
    document's own words. Look for name-calling, insults, or labels the writer applies to people; \
    blaming or criticising former employers, managers, or coworkers; self-deprecation and other \
    unprofessional tone; objectives that say what the writer wants instead of what they offer; \
    empty buzzwords, exaggerated superlatives, and unsupported traits; irrelevant personal details \
    or hobbies; and private information such as age, marital status, or a photograph. Also flag a \
    duty-only line whose only problem is that it shows no result, but prefer a concrete example of \
    bad tone or wording over a general complaint. Each flag needs a short `label` naming the \
    practice, a `why` of at most 180 characters explaining the cost to a reader, a `fix` of at most \
    160 characters telling the writer what to do, and one `evidence` quote copied verbatim from a \
    section in the user message with that section's exact id. Never invent, infer, or reword a \
    quote, never flag a neutral fact or a skill, and never use an empty label. If nothing clearly \
    qualifies, emit no flags. Do not flag a practice twice, and report at most four flags.";

/// One flag field event shape used in the combined NDJSON response.
pub const ISSUE_EVENT_SHAPE: &str = "{\"type\":\"field\",\"name\":\"issue\",\"value\":{\"label\":\"Blaming a former manager\",\"why\":\"It puts the focus on a person instead of your work.\",\"fix\":\"Describe the process gap without naming anyone.\",\"evidence\":{\"section_id\":\"experience-1\",\"quote\":\"My old manager never explained the process.\"}}}";

/// Extract and ground the model's writing flags. An empty result is valid: a
/// clean résumé needs no warnings. Ungrounded flags are discarded rather than
/// failing the whole upload.
#[must_use]
pub fn extract(output: &StreamOutput, sections: &SectionMap, model: &str) -> WritingIssueSet {
    WritingIssueSet {
        issues: validate(final_issues(output), sections),
        engine: "chat".to_owned(),
        model: model.to_owned(),
    }
}

#[derive(Deserialize, Serialize)]
struct RawEvidence {
    section_id: String,
    quote: String,
}

#[derive(Deserialize, Serialize)]
struct RawIssue {
    label: String,
    why: String,
    fix: String,
    evidence: RawEvidence,
}

/// Prefer streamed `issue` field events, then a single whole-object reply.
fn final_issues(output: &StreamOutput) -> Vec<RawIssue> {
    let mut raw = Vec::new();
    for event in &output.events {
        if let ModelEvent::Field { name, value } = event {
            if name == "issue" {
                if let Ok(issue) = serde_json::from_value::<RawIssue>(value.clone()) {
                    raw.push(issue);
                }
            } else if name == "issues"
                && let Ok(list) = serde_json::from_value::<Vec<RawIssue>>(value.clone())
            {
                raw.extend(list);
            }
        }
    }
    if !raw.is_empty() {
        return raw;
    }
    let Some(json) = json_object(&output.raw) else {
        return raw;
    };
    match json.get("issues") {
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                if let Ok(issue) = serde_json::from_value::<RawIssue>(item.clone()) {
                    raw.push(issue);
                }
            }
        }
        Some(value) => {
            if let Ok(issue) = serde_json::from_value::<RawIssue>(value.clone()) {
                raw.push(issue);
            }
        }
        None => {}
    }
    raw
}

fn json_object(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();
    if let Ok(json) = serde_json::from_str(trimmed) {
        return Some(json);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if start >= end {
        return None;
    }
    serde_json::from_str(&trimmed[start..=end]).ok()
}

fn validate(raw: Vec<RawIssue>, sections: &SectionMap) -> Vec<WritingIssue> {
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    for issue in raw {
        let label = clean(&issue.label, MAX_LABEL_CHARS);
        let why = clean(&issue.why, MAX_WHY_CHARS);
        let fix = clean(&issue.fix, MAX_FIX_CHARS);
        if label.is_empty() || why.is_empty() || fix.is_empty() {
            continue;
        }
        let quote = issue.evidence.quote.trim();
        if quote.is_empty() || quote.len() > MAX_QUOTE_CHARS {
            continue;
        }
        let Some(section) = sections
            .sections
            .iter()
            .find(|section| section.id == issue.evidence.section_id)
        else {
            continue;
        };
        if !normalize(&section.body).contains(&normalize(quote)) {
            continue;
        }
        let key = (
            normalize(&label),
            issue.evidence.section_id.clone(),
            normalize(quote),
        );
        if !seen.insert(key) {
            continue;
        }
        issues.push(WritingIssue {
            label,
            why,
            fix,
            section_id: issue.evidence.section_id,
            quote: quote.to_owned(),
        });
        if issues.len() >= MAX_ISSUES {
            break;
        }
    }
    issues
}

fn clean(text: &str, limit: usize) -> String {
    truncate(
        &text.split_whitespace().collect::<Vec<_>>().join(" "),
        limit,
    )
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn truncate(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{DetectedSection, SectionKind};

    fn sections() -> SectionMap {
        SectionMap {
            engine: "test".to_owned(),
            model: "test".to_owned(),
            sections: vec![DetectedSection {
                id: "experience-1".to_owned(),
                kind: SectionKind::Experience,
                heading: "Experience".to_owned(),
                body: "My old manager never explained the process.\nDelivered 12 projects."
                    .to_owned(),
                confidence: 0.9,
                feedback: None,
            }],
        }
    }

    fn issue(label: &str, quote: &str) -> serde_json::Value {
        serde_json::json!({
            "label": label,
            "why": "It puts the focus on a person instead of your work.",
            "fix": "Describe the process gap without naming anyone.",
            "evidence": { "section_id": "experience-1", "quote": quote },
        })
    }

    #[test]
    fn rules_name_the_practices_the_writer_asked_about() {
        assert!(ISSUE_RULES.contains("name-calling"));
        assert!(ISSUE_RULES.contains("blaming"));
        assert!(ISSUE_RULES.contains("unprofessional tone"));
        assert!(ISSUE_RULES.contains("verbatim"));
    }

    #[test]
    fn streamed_flags_are_grounded_and_limited() {
        let quote = "My old manager never explained the process.";
        let mut output = StreamOutput::default();
        for label in [
            "Blame",
            "Name-calling",
            "Buzzwords",
            "Self-deprecation",
            "Objective",
            "Tone",
        ] {
            output.apply(&ModelEvent::Field {
                name: "issue".to_owned(),
                value: issue(label, quote),
            });
        }
        let flags = extract(&output, &sections(), "test");
        assert_eq!(flags.engine, "chat");
        assert_eq!(flags.model, "test");
        assert_eq!(flags.issues.len(), MAX_ISSUES);
        assert_eq!(flags.issues[0].quote, quote);
        assert_eq!(
            flags.issues[0].fix,
            "Describe the process gap without naming anyone."
        );
    }

    #[test]
    fn invented_and_duplicate_flags_are_dropped() {
        let mut output = StreamOutput::default();
        output.apply(&ModelEvent::Field {
            name: "issue".to_owned(),
            value: issue("Blame", "A line the model made up."),
        });
        output.apply(&ModelEvent::Field {
            name: "issue".to_owned(),
            value: issue("Blame", "My old manager never explained the process."),
        });
        output.apply(&ModelEvent::Field {
            name: "issue".to_owned(),
            value: issue("Blame", "  My   old manager never explained the process.  "),
        });
        assert_eq!(extract(&output, &sections(), "test").issues.len(), 1);
    }

    #[test]
    fn whole_object_replies_still_produce_flags() {
        let output = StreamOutput {
            raw: serde_json::json!({
                "issues": [issue("Blame", "My old manager never explained the process.")]
            })
            .to_string(),
            ..StreamOutput::default()
        };
        assert_eq!(extract(&output, &sections(), "test").issues.len(), 1);
    }

    #[test]
    fn a_clean_reply_has_no_flags() {
        assert_eq!(
            extract(&StreamOutput::default(), &sections(), "test").issues,
            [] as [WritingIssue; 0]
        );
    }
}
