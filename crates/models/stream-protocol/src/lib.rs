//! The small NDJSON language both generators speak, and the processor that
//! turns a chat model's stream into usable events.
//!
//! A model writes one JSON object per line and nothing else:
//!
//! ```text
//! {"type":"stage","label":"reading"}
//! {"type":"delta","field":"profile_text","value":"You have "}
//! {"type":"field","name":"is_resume","value":true}
//! {"type":"done"}
//! ```
//!
//! Lines are deliberately tiny: a local model can produce them reliably, and
//! `NdjsonProcessor` can parse them as they arrive, even when a chunk ends in
//! the middle of a JSON string. The same processor serves the summary and the
//! diagnosis flows; only the field names differ.
//!
//! Models that ignore the format are still supported. `infer_stream` returns
//! the raw text it saw alongside any structured events, so generators can fall
//! back to their previous whole-blob parsing.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::time::Duration;

use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One line of the model's output, after parsing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelEvent {
    /// A short Claude-style status line, e.g. `reading` or `comparing`.
    Stage { label: String },
    /// A piece of the prose. Concatenate values with the same `field`.
    Delta { field: String, value: String },
    /// A scalar fact that arrives outside the prose (summary's `is_resume`).
    Field { name: String, value: Value },
    /// The model has finished. Always the last line when present.
    Done,
    /// A problem worth showing to the operator instead of half a diagnosis.
    Error { message: String },
}

/// Where parsed events go. Implemented by the worker to forward frames to the
/// live SSE bus, and by tests to collect them.
pub trait Emitter: Send + Sync {
    fn emit(&self, event: &ModelEvent);
}

/// Useful for warmups and tests that only care about the final result.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopEmitter;

impl Emitter for NoopEmitter {
    fn emit(&self, _: &ModelEvent) {}
}

impl<F> Emitter for F
where
    F: Fn(&ModelEvent) + Send + Sync,
{
    fn emit(&self, event: &ModelEvent) {
        self(event);
    }
}

/// Incremental NDJSON parser. Feed it arbitrary text chunks; it returns one
/// event per complete line and holds a partial trailing line until the next
/// chunk (or `finish`) completes it.
#[derive(Debug, Default)]
pub struct NdjsonProcessor {
    pending: String,
}

impl NdjsonProcessor {
    pub fn push(&mut self, chunk: &str) -> Vec<ModelEvent> {
        self.pending.push_str(chunk);
        let mut events = Vec::new();
        while let Some(index) = self.pending.find('\n') {
            let line: String = self.pending.drain(..=index).collect();
            if let Some(event) = parse_line(&line) {
                events.push(event);
            }
        }
        events
    }

    /// Parse a final line that had no trailing newline.
    pub fn finish(&mut self) -> Vec<ModelEvent> {
        let line = std::mem::take(&mut self.pending);
        parse_line(&line).into_iter().collect()
    }
}

fn parse_line(line: &str) -> Option<ModelEvent> {
    let mut value = line.trim();
    if let Some(data) = value.strip_prefix("data:") {
        value = data.trim();
    }
    if value.is_empty() || value.starts_with("```") || value == "[DONE]" {
        return None;
    }
    serde_json::from_str(value).ok()
}

/// Everything a generator needs to build the final, persisted result.
#[derive(Debug, Default)]
pub struct StreamOutput {
    /// Concatenated model text, including lines that were not valid NDJSON.
    /// Used by the legacy fallback path.
    pub raw: String,
    pub events: Vec<ModelEvent>,
    pub fields: BTreeMap<String, Value>,
    /// Concatenated text from `delta` events and from `field` events whose value
    /// is a string. Small models often put whole paragraphs in a `field`.
    pub text: BTreeMap<String, String>,
    pub stages: Vec<String>,
    pub done: bool,
    /// True when at least one NDJSON event was recognised.
    pub structured: bool,
}

impl StreamOutput {
    pub fn apply(&mut self, event: &ModelEvent) {
        self.events.push(event.clone());
        self.structured = true;
        match event {
            ModelEvent::Stage { label } => self.stages.push(label.clone()),
            ModelEvent::Delta { field, value } => {
                self.text.entry(field.clone()).or_default().push_str(value);
            }
            ModelEvent::Field { name, value } => {
                self.fields.insert(name.clone(), value.clone());
                if let Some(text) = value.as_str() {
                    self.text.entry(name.clone()).or_default().push_str(text);
                }
            }
            ModelEvent::Done => self.done = true,
            ModelEvent::Error { .. } => {}
        }
    }

    /// The concatenated value of every delta with this field name.
    pub fn text(&self, field: &str) -> String {
        self.text.get(field).cloned().unwrap_or_default()
    }

    pub fn field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }
}

/// Send a chat request and read its answer as it is produced.
///
/// Streaming chat servers answer with Server-Sent Events. Older/OpenAI-style
/// sinks may answer with a single JSON document even when `stream: true` was
/// requested; both are accepted. Retries happen only before the first byte of
/// a response is read, so a failed mid-stream does not duplicate tokens.
pub fn infer_stream(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    request: &impl Serialize,
    emitter: &dyn Emitter,
) -> Result<StreamOutput, String> {
    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let response = client
            .post(endpoint)
            .header(ACCEPT, "text/event-stream")
            .json(request)
            .send();
        let response = match response {
            Ok(response) => response,
            Err(err) => {
                last_err = format!("chat request to {endpoint}: {err}");
                if is_transient(&err) && attempt < MAX_ATTEMPTS {
                    std::thread::sleep(Duration::from_millis(100 * (2u64.pow(attempt - 1))));
                    continue;
                }
                return Err(last_err);
            }
        };
        let response = match response.error_for_status() {
            Ok(response) => response,
            Err(err) => {
                let status = err.status().map(|value| value.as_u16()).unwrap_or(0);
                last_err = format!("chat request to {endpoint}: {err}");
                if status >= 500 && attempt < MAX_ATTEMPTS {
                    std::thread::sleep(Duration::from_millis(100 * (2u64.pow(attempt - 1))));
                    continue;
                }
                return Err(last_err);
            }
        };
        return read_chat_stream(response, emitter);
    }
    Err(last_err)
}

fn read_chat_stream(
    response: reqwest::blocking::Response,
    emitter: &dyn Emitter,
) -> Result<StreamOutput, String> {
    let mut processor = NdjsonProcessor::default();
    let mut output = StreamOutput::default();
    let mut reader = BufReader::new(response);
    let mut line = String::new();
    let mut sse = false;
    let mut json = String::new();

    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|err| format!("read chat stream: {err}"))?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !sse
            && (trimmed.starts_with("data:")
                || trimmed.starts_with("event:")
                || trimmed.starts_with(':'))
        {
            sse = true;
        }
        if sse {
            let Some(data) = trimmed.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            let Ok(value) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            if let Some(content) = chunk_content(&value) {
                push_content(&mut output, &mut processor, emitter, &content);
            }
        } else {
            json.push_str(trimmed);
        }
    }

    if !sse && !json.is_empty() {
        let value: Value =
            serde_json::from_str(&json).map_err(|err| format!("decode chat response: {err}"))?;
        let content =
            chunk_content(&value).ok_or_else(|| "chat response had no content".to_owned())?;
        push_content(&mut output, &mut processor, emitter, &content);
    }

    for event in processor.finish() {
        emitter.emit(&event);
        output.apply(&event);
    }
    Ok(output)
}

fn push_content(
    output: &mut StreamOutput,
    processor: &mut NdjsonProcessor,
    emitter: &dyn Emitter,
    content: &str,
) {
    output.raw.push_str(content);
    for event in processor.push(content) {
        emitter.emit(&event);
        output.apply(&event);
    }
}

/// Both OpenAI-compatible SSE chunks and single-shot chat responses put the
/// generated text in `choices[0].delta.content` or `choices[0].message.content`.
fn chunk_content(value: &Value) -> Option<String> {
    let choice = value.get("choices")?.as_array()?.first()?;
    choice
        .get("delta")
        .and_then(|delta| delta.get("content"))
        .or_else(|| {
            choice
                .get("message")
                .and_then(|message| message.get("content"))
        })
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn is_transient(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout() || err.is_request()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(output: &StreamOutput) -> Vec<String> {
        output
            .events
            .iter()
            .map(|event| serde_json::to_string(event).unwrap())
            .collect()
    }

    #[test]
    fn processor_waits_for_complete_lines_and_splits_chunks_mid_event() {
        let mut processor = NdjsonProcessor::default();
        let first = processor.push("{\"type\":\"stage\",\"label\":\"rea");
        assert!(first.is_empty());
        let second =
            processor.push("ding\"}\n{\"type\":\"delta\",\"field\":\"text\",\"value\":\"he");
        assert_eq!(
            second,
            vec![ModelEvent::Stage {
                label: "reading".to_owned()
            }]
        );
        let third = processor.push("llo\"}\n{\"type\":\"done\"}");
        assert_eq!(
            third,
            vec![ModelEvent::Delta {
                field: "text".to_owned(),
                value: "hello".to_owned()
            }]
        );
        assert_eq!(processor.finish(), vec![ModelEvent::Done]);
    }

    #[test]
    fn processor_ignores_fences_prefixes_and_malformed_lines() {
        let mut processor = NdjsonProcessor::default();
        let events = processor.push(
            "```json\n{\"type\":\"stage\",\"label\":\"reading\"}\nnot json\n{\"oops\"\ndata: {\"type\":\"done\"}\n",
        );
        assert_eq!(
            events,
            vec![
                ModelEvent::Stage {
                    label: "reading".to_owned()
                },
                ModelEvent::Done
            ]
        );
    }

    #[test]
    fn output_accumulates_fields_text_and_stages() {
        let mut output = StreamOutput::default();
        output.apply(&ModelEvent::Field {
            name: "is_resume".to_owned(),
            value: serde_json::json!(true),
        });
        output.apply(&ModelEvent::Delta {
            field: "profile_text".to_owned(),
            value: "You build ".to_owned(),
        });
        output.apply(&ModelEvent::Delta {
            field: "profile_text".to_owned(),
            value: "systems.".to_owned(),
        });
        output.apply(&ModelEvent::Stage {
            label: "writing".to_owned(),
        });
        assert_eq!(output.text("profile_text"), "You build systems.");
        assert_eq!(output.field("is_resume"), Some(&serde_json::json!(true)));
        assert_eq!(output.stages, vec!["writing"]);
        assert!(output.structured);
        assert!(!events(&output).is_empty());
    }

    #[test]
    fn string_field_values_also_accumulate_as_text() {
        let mut output = StreamOutput::default();
        output.apply(&ModelEvent::Field {
            name: "profile_text".to_owned(),
            value: serde_json::json!("You build "),
        });
        output.apply(&ModelEvent::Field {
            name: "profile_text".to_owned(),
            value: serde_json::json!("systems."),
        });
        output.apply(&ModelEvent::Field {
            name: "is_resume".to_owned(),
            value: serde_json::json!(true),
        });
        assert_eq!(output.text("profile_text"), "You build systems.");
        assert_eq!(output.text("is_resume"), "");
        assert_eq!(output.field("is_resume"), Some(&serde_json::json!(true)));
    }
}
