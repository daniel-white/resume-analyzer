//! Section detection for the resume details page.
//!
//! A resume is a small, ordered list of text lines. The detector finds
//! heading-shaped lines, maps each one onto a canonical [`SectionKind`], and
//! groups the lines beneath it. The default backend is a fastembed sentence
//! embedding model (BGE small) that scores a heading against canonical labels.
//! If the model is not available — CI, a cold laptop, a cluster without the
//! model volume — a deterministic alias classifier produces the same shape.
//!
//! The two backends deliberately share the candidate and segmentation code, so
//! only the label decision changes. A missing model degrades accuracy, not the
//! contract.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use domain::{DetectedSection, SectionKind, SectionMap};
use fastembed::{
    EmbeddingModel, InitOptions, InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding,
    TokenizerFiles, UserDefinedEmbeddingModel,
};

/// A heading this short is unlikely to be a sentence of body text.
const MAX_HEADING_CHARS: usize = 80;
const MAX_HEADING_WORDS: usize = 7;
/// Keep one pathological upload from filling the page and the database.
const MAX_SECTION_BODY_CHARS: usize = 20_000;
const MAX_LINES: usize = 500;

const ENGINES: &str = "section-detector-v1";
const EMBEDDING_ENGINE: &str = "fastembed";
const EMBEDDING_MODEL: &str = "BAAI/bge-small-en-v1.5";

/// Canonical labels and the heading spellings people actually use. The
/// embedding backend uses these as prompts, so adding an alias improves both
/// backends at once.
const ALIASES: &[(SectionKind, &[&str])] = &[
    (
        SectionKind::Summary,
        &[
            "summary",
            "professional summary",
            "career summary",
            "profile",
            "objective",
            "career objective",
            "about",
            "about me",
        ],
    ),
    (
        SectionKind::Experience,
        &[
            "experience",
            "work experience",
            "professional experience",
            "employment",
            "employment history",
            "work history",
            "career history",
            "relevant experience",
        ],
    ),
    (
        SectionKind::Education,
        &[
            "education",
            "academic background",
            "academics",
            "education and training",
            "qualifications",
        ],
    ),
    (
        SectionKind::Skills,
        &[
            "skills",
            "technical skills",
            "core skills",
            "core competencies",
            "competencies",
            "technologies",
            "tech stack",
            "tools",
        ],
    ),
    (
        SectionKind::Projects,
        &[
            "projects",
            "selected projects",
            "personal projects",
            "side projects",
            "portfolio",
        ],
    ),
    (
        SectionKind::Certifications,
        &[
            "certifications",
            "certificates",
            "licenses",
            "certifications and licenses",
        ],
    ),
    (
        SectionKind::Awards,
        &[
            "awards",
            "honors",
            "awards and honors",
            "achievements",
            "accomplishments",
        ],
    ),
    (SectionKind::Languages, &["languages", "language skills"]),
    (
        SectionKind::Publications,
        &["publications", "papers", "research", "presentations"],
    ),
    (
        SectionKind::Volunteer,
        &[
            "volunteer",
            "volunteering",
            "volunteer experience",
            "community involvement",
        ],
    ),
    (
        SectionKind::Interests,
        &[
            "interests",
            "hobbies",
            "activities",
            "hobbies and interests",
        ],
    ),
    (
        SectionKind::Contact,
        &[
            "contact",
            "contact information",
            "contact details",
            "personal information",
            "details",
        ],
    ),
];

/// A configured detector. Cloning shares the lazily loaded model.
pub struct SectionDetector {
    mode: Mode,
    loaded: Arc<OnceLock<Option<Mutex<EmbeddingState>>>>,
    engine: String,
    model: String,
}

#[derive(Clone)]
enum Mode {
    Heuristic,
    Embedding {
        model_dir: Option<PathBuf>,
        cache_dir: PathBuf,
    },
}

struct EmbeddingState {
    model: TextEmbedding,
    /// One embedding per canonical alias, paired with its kind.
    labels: Vec<(SectionKind, Vec<f32>)>,
}

impl Clone for SectionDetector {
    fn clone(&self) -> Self {
        Self {
            mode: self.mode.clone(),
            // The model is shared with the clone, so a worker and the API do
            // not load it twice.
            loaded: Arc::clone(&self.loaded),
            engine: self.engine.clone(),
            model: self.model.clone(),
        }
    }
}

impl SectionDetector {
    /// No model files, no downloads. Used by tests and when explicitly disabled.
    pub fn heuristic() -> Self {
        Self {
            mode: Mode::Heuristic,
            loaded: Arc::new(OnceLock::new()),
            engine: ENGINES.to_owned(),
            model: "heading-alias-v1".to_owned(),
        }
    }

    /// Read `SECTION_MODEL` (`auto` by default) and `SECTION_MODEL_DIR`.
    /// A missing or broken model logs nothing and falls back to aliases; the
    /// worker still stores a section map.
    pub fn from_env() -> Self {
        let choice = env::var("SECTION_MODEL")
            .unwrap_or_else(|_| "auto".to_owned())
            .to_ascii_lowercase();
        if matches!(choice.as_str(), "off" | "none" | "heuristic" | "disabled") {
            return Self::heuristic();
        }
        let model_dir = env::var("SECTION_MODEL_DIR")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let cache_dir = env::var("FASTEMBED_CACHE_DIR")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".fastembed_cache"));
        let model = env::var("SECTION_MODEL_NAME")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| EMBEDDING_MODEL.to_owned());
        Self {
            mode: Mode::Embedding {
                model_dir,
                cache_dir,
            },
            loaded: Arc::new(OnceLock::new()),
            engine: EMBEDDING_ENGINE.to_owned(),
            model,
        }
    }

    /// The engine name shown in the UI. It reports the heuristic fallback when
    /// the embedding model could not be loaded.
    pub fn engine(&self) -> &str {
        if self.mode().is_some() {
            &self.engine
        } else {
            ENGINES
        }
    }

    pub fn model(&self) -> &str {
        if self.mode().is_some() {
            &self.model
        } else {
            "heading-alias-v1"
        }
    }

    /// Load the model now instead of on the first upload. Safe to call more
    /// than once; a failed load stays failed and falls back to aliases.
    pub fn warm(&self) {
        let _ = self.state();
    }

    /// Detect sections from ordered text lines.
    pub fn detect(&self, lines: &[String]) -> SectionMap {
        let lines: Vec<_> = lines
            .iter()
            .map(|text| domain::DocumentLine {
                text: text.clone(),
                ..Default::default()
            })
            .collect();
        self.detect_document(&lines)
    }

    /// Formatting supports a label decision; it never supplies the label itself.
    pub fn detect_document(&self, source: &[domain::DocumentLine]) -> SectionMap {
        let mut sizes: Vec<_> = source
            .iter()
            .take(MAX_LINES)
            .filter_map(|line| {
                line.font_size
                    .filter(|size| size.is_finite() && *size > 0.)
                    .map(|size| (size, line.text.chars().count().min(200)))
            })
            .collect();
        sizes.sort_by(|a, b| a.0.total_cmp(&b.0));
        let midpoint = sizes.iter().map(|(_, weight)| weight).sum::<usize>() / 2;
        let mut weight = 0;
        let normal_size = sizes
            .iter()
            .find(|(_, count)| {
                weight += count;
                weight > midpoint
            })
            .map(|(size, _)| *size);
        let mut lines = Vec::new();
        let mut hints = Vec::new();
        for line in source.iter().take(MAX_LINES) {
            if let Some(text) = clean_lines(std::slice::from_ref(&line.text)).pop() {
                lines.push(repair_initial(&text));
                hints.push(
                    line.emphasized
                        || line
                            .font_size
                            .zip(normal_size)
                            .is_some_and(|(size, normal)| size >= normal * 1.2),
                );
            }
        }
        let state = self.state();
        let headings = find_headings(&lines, &hints, state);
        let sections = segment(&lines, &headings);
        SectionMap {
            engine: if state.is_some() {
                self.engine.clone()
            } else {
                ENGINES.to_owned()
            },
            model: if state.is_some() {
                self.model.clone()
            } else {
                "heading-alias-v1".to_owned()
            },
            sections,
        }
    }

    fn mode(&self) -> Option<&Mode> {
        match &self.mode {
            Mode::Heuristic => None,
            mode => Some(mode),
        }
    }

    fn state(&self) -> Option<&Mutex<EmbeddingState>> {
        let Mode::Embedding { .. } = self.mode else {
            return None;
        };
        self.loaded
            .get_or_init(|| match self.load_model() {
                Ok(state) => Some(Mutex::new(state)),
                Err(err) => {
                    eprintln!("section model unavailable: {err}; using heading aliases");
                    None
                }
            })
            .as_ref()
    }

    fn load_model(&self) -> Result<EmbeddingState, String> {
        let Mode::Embedding {
            model_dir,
            cache_dir,
        } = &self.mode
        else {
            return Err("heuristic mode".to_owned());
        };
        let model = match model_dir {
            Some(dir) if dir.is_dir() => load_local_model(dir)?,
            Some(dir) => {
                return Err(format!(
                    "SECTION_MODEL_DIR {} is not a directory",
                    dir.display()
                ));
            }
            None => TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::BGESmallENV15)
                    .with_cache_dir(cache_dir.clone())
                    .with_show_download_progress(false),
            )
            .map_err(|err| format!("load {EMBEDDING_MODEL}: {err}"))?,
        };
        let prompts: Vec<String> = ALIASES
            .iter()
            .flat_map(|(_, aliases)| aliases.iter().map(|alias| (*alias).to_owned()))
            .collect();
        let vectors = model
            .embed(prompts, None)
            .map_err(|err| format!("embed section labels: {err}"))?;
        let mut labels = Vec::with_capacity(vectors.len());
        let mut index = 0;
        for (kind, aliases) in ALIASES {
            for _ in *aliases {
                labels.push((*kind, vectors[index].clone()));
                index += 1;
            }
        }
        Ok(EmbeddingState { model, labels })
    }
}

impl std::fmt::Debug for SectionDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SectionDetector")
            .field("engine", &self.engine())
            .field("model", &self.model())
            .finish()
    }
}

fn load_local_model(dir: &Path) -> Result<TextEmbedding, String> {
    let read = |name: &str| {
        fs::read(dir.join(name)).map_err(|err| format!("read {}: {err}", dir.join(name).display()))
    };
    let model_file = ["model.onnx", "onnx/model.onnx"]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| format!("{} has no model.onnx", dir.display()))?;
    let onnx =
        fs::read(&model_file).map_err(|err| format!("read {}: {err}", model_file.display()))?;
    let pool = env::var("SECTION_MODEL_POOLING")
        .unwrap_or_else(|_| "cls".to_owned())
        .to_ascii_lowercase();
    let pooling = if pool == "mean" {
        Pooling::Mean
    } else {
        Pooling::Cls
    };
    let quantization = env::var("SECTION_MODEL_QUANTIZED")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let model = UserDefinedEmbeddingModel::new(
        onnx,
        TokenizerFiles {
            tokenizer_file: read("tokenizer.json")?,
            config_file: read("config.json")?,
            special_tokens_map_file: read("special_tokens_map.json")?,
            tokenizer_config_file: read("tokenizer_config.json")?,
        },
    )
    .with_pooling(pooling)
    .with_quantization(if quantization {
        QuantizationMode::Static
    } else {
        QuantizationMode::None
    });
    TextEmbedding::try_new_from_user_defined(model, InitOptionsUserDefined::new())
        .map_err(|err| format!("load local model: {err}"))
}

/// One heading decision before segmentation.
struct Heading {
    index: usize,
    kind: SectionKind,
    confidence: f32,
}

fn find_headings(
    lines: &[String],
    hints: &[bool],
    state: Option<&Mutex<EmbeddingState>>,
) -> Vec<Heading> {
    let candidates: Vec<(usize, &String)> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| alias_kind(line).is_some() || heading_shaped(line))
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }

    // Alias matches are high precision, so resolve them first. Only candidates
    // without a lexical match need the model.
    let mut decisions: HashMap<usize, (SectionKind, f32)> = HashMap::new();
    let mut learned = Vec::new();
    for (index, line) in &candidates {
        if let Some((kind, confidence)) = alias_kind(line) {
            decisions.insert(*index, (kind, confidence));
        } else {
            learned.push((*index, (*line).clone()));
        }
    }

    if let Some(state) = state
        && !learned.is_empty()
        && let Ok(state) = state.lock()
    {
        let texts: Vec<String> = learned.iter().map(|(_, line)| line.clone()).collect();
        if let Ok(vectors) = state.model.embed(texts, None) {
            for ((index, _), vector) in learned.into_iter().zip(vectors) {
                if let Some((kind, confidence)) =
                    embedding_kind(&vector, &state.labels, hints[index])
                    && body_supports(kind, &lines[index + 1..])
                {
                    decisions.insert(index, (kind, confidence));
                }
            }
        }
    }

    let mut headings: Vec<Heading> = decisions
        .into_iter()
        .map(|(index, (kind, confidence))| Heading {
            index,
            kind,
            confidence,
        })
        .collect();
    headings.sort_by_key(|heading| heading.index);
    // Two headings back to back with the same kind is a formatting artifact,
    // not two sections. Keep the first.
    headings
        .dedup_by(|next, previous| next.kind == previous.kind && next.index == previous.index + 1);
    headings
}

fn heading_shaped(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty() || line.chars().count() > MAX_HEADING_CHARS {
        return false;
    }
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.is_empty() || words.len() > MAX_HEADING_WORDS {
        return false;
    }
    if line.contains('@')
        || line.starts_with('•')
        || line.starts_with('-')
        || line.starts_with('*')
        || line.ends_with('.')
        || line.ends_with(',')
        || line.ends_with(';')
    {
        return false;
    }
    // Job dates and phone numbers are not headings.
    if line.chars().filter(char::is_ascii_digit).count() > 3 {
        return false;
    }
    // "Experience" and "WORK EXPERIENCE" are common; body prose almost always
    // has more words. A title-cased or upper-cased line is a stronger hint.
    line.chars().any(char::is_alphabetic)
}

/// Semantic similarity alone can mistake job titles and skill names for
/// headings. Require independent evidence in the following body for learned
/// labels; exact aliases do not need this language-dependent check.
fn body_supports(kind: SectionKind, lines: &[String]) -> bool {
    let body = lines
        .iter()
        .take(5)
        .take_while(|line| alias_kind(line).is_none())
        .map(|line| normalize(line))
        .collect::<Vec<_>>()
        .join(" ");
    let cues: &[&str] = match kind {
        SectionKind::Experience => &[
            "engineer",
            "manager",
            "developed",
            "built",
            "led",
            "present",
        ],
        SectionKind::Education => &[
            "university",
            "college",
            "bachelor",
            "master",
            "degree",
            "diploma",
            "bsc",
            "msc",
            "phd",
        ],
        SectionKind::Skills => &[
            "rust",
            "python",
            "java",
            "javascript",
            "typescript",
            "sql",
            "excel",
            "leadership",
            "communication",
        ],
        SectionKind::Projects => &["built", "developed", "github", "application", "project"],
        SectionKind::Certifications => &["certified", "certificate", "certification", "license"],
        SectionKind::Languages => &["english", "spanish", "french", "german", "native", "fluent"],
        SectionKind::Contact => return looks_like_contact(&lines[..lines.len().min(5)]),
        SectionKind::Summary => &[
            "professional",
            "experienced",
            "years",
            "seeking",
            "specialist",
        ],
        SectionKind::Awards => &["award", "winner", "honor", "prize", "scholarship"],
        SectionKind::Publications => &["journal", "published", "doi", "conference"],
        SectionKind::Volunteer => &["volunteer", "community", "nonprofit", "charity"],
        SectionKind::Interests => &["reading", "hiking", "music", "travel", "sports"],
        SectionKind::Other => return false,
    };
    body.split_whitespace().any(|word| cues.contains(&word))
}

fn alias_kind(line: &str) -> Option<(SectionKind, f32)> {
    if line.chars().count() > MAX_HEADING_CHARS {
        return None;
    }
    let normalized = normalize(line);
    if normalized.is_empty() {
        return None;
    }
    for (kind, aliases) in ALIASES {
        for alias in *aliases {
            if normalized == *alias {
                return Some((*kind, 0.97));
            }
            // Letter-spaced headings and stray spaces inside extracted words
            // are safe to repair against a known label, not arbitrary prose.
            if normalized.replace(' ', "") == alias.replace(' ', "") {
                return Some((*kind, 0.9));
            }
            // Accept a trailing plural/singular variation without accepting a
            // longer phrase that merely contains a label.
            if normalized == format!("{alias}s") || normalized == alias.trim_end_matches('s') {
                return Some((*kind, 0.9));
            }
        }
    }
    None
}

fn embedding_kind(
    vector: &[f32],
    labels: &[(SectionKind, Vec<f32>)],
    formatted: bool,
) -> Option<(SectionKind, f32)> {
    let mut best: HashMap<SectionKind, f32> = HashMap::new();
    for (kind, label) in labels {
        let score = cosine(vector, label);
        best.entry(*kind)
            .and_modify(|current| *current = current.max(score))
            .or_insert(score);
    }
    let mut ranked: Vec<(SectionKind, f32)> = best.into_iter().collect();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    let (kind, score) = *ranked.first()?;
    let second = ranked.get(1).map(|(_, score)| *score).unwrap_or(0.0);
    // A close second means the heading is ambiguous; leave it for the alias
    // fallback rather than inventing a section.
    let threshold = if formatted { 0.58 } else { 0.62 };
    if score < threshold || score - second < 0.02 {
        return None;
    }
    Some((kind, (0.6 + (score - 0.62) * 1.4).clamp(0.6, 0.96)))
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let dot: f64 = left
        .iter()
        .zip(right)
        .map(|(left, right)| f64::from(*left) * f64::from(*right))
        .sum();
    let left_norm: f64 = left
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();
    let right_norm: f64 = right
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        (dot / (left_norm * right_norm)).clamp(-1.0, 1.0) as f32
    }
}

fn segment(lines: &[String], headings: &[Heading]) -> Vec<DetectedSection> {
    if lines.is_empty() {
        return Vec::new();
    }

    // No heading at all: the file is one unlabelled block. A top block that
    // carries an email or phone number is the contact card; anything else is
    // simply the resume body.
    if headings.is_empty() {
        let body = join_body(lines);
        if body.is_empty() {
            return Vec::new();
        }
        let contact = looks_like_contact(lines);
        return vec![DetectedSection {
            id: section_id(
                if contact {
                    SectionKind::Contact
                } else {
                    SectionKind::Other
                },
                1,
            ),
            kind: if contact {
                SectionKind::Contact
            } else {
                SectionKind::Other
            },
            heading: if contact { "Contact" } else { "Resume" }.to_owned(),
            body,
            confidence: if contact { 0.75 } else { 0.4 },
            feedback: None,
        }];
    }

    let mut sections = Vec::new();

    // Text above the first heading is the name/contact block. Treat it as one
    // section instead of dropping it.
    let first = headings.first().map(|heading| heading.index).unwrap_or(0);
    if first > 0 {
        let body = join_body(&lines[..first]);
        if !body.is_empty() {
            let contact = looks_like_contact(&lines[..first]);
            let kind = if contact {
                SectionKind::Contact
            } else {
                SectionKind::Other
            };
            sections.push(DetectedSection {
                id: section_id(kind, 1),
                kind,
                heading: if contact { "Contact" } else { "Resume" }.to_owned(),
                body,
                confidence: if contact { 0.75 } else { 0.4 },
                feedback: None,
            });
        }
    }

    let mut counts: HashMap<SectionKind, usize> = HashMap::new();
    for (position, heading) in headings.iter().enumerate() {
        let end = headings
            .get(position + 1)
            .map(|next| next.index)
            .unwrap_or(lines.len());
        let body = join_body(&lines[heading.index + 1..end]);

        // A repeated heading of the same kind is a formatting artifact. Fold
        // its lines into the section we already opened.
        if let Some(last) = sections.last_mut()
            && last.kind == heading.kind
        {
            if !body.is_empty() {
                if !last.body.is_empty() {
                    last.body.push('\n');
                }
                last.body.push_str(&body);
                truncate_chars(&mut last.body, MAX_SECTION_BODY_CHARS);
            }
            last.confidence = last.confidence.max(heading.confidence);
            continue;
        }

        let count = counts
            .entry(heading.kind)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        sections.push(DetectedSection {
            id: section_id(heading.kind, *count),
            kind: heading.kind,
            heading: lines[heading.index].clone(),
            body,
            confidence: heading.confidence,
            feedback: None,
        });
    }
    sections
}

fn section_id(kind: SectionKind, occurrence: usize) -> String {
    format!("{}-{occurrence}", kind_key(kind))
}

fn kind_key(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Summary => "summary",
        SectionKind::Experience => "experience",
        SectionKind::Education => "education",
        SectionKind::Skills => "skills",
        SectionKind::Projects => "projects",
        SectionKind::Certifications => "certifications",
        SectionKind::Awards => "awards",
        SectionKind::Languages => "languages",
        SectionKind::Publications => "publications",
        SectionKind::Volunteer => "volunteer",
        SectionKind::Interests => "interests",
        SectionKind::Contact => "contact",
        SectionKind::Other => "other",
    }
}

fn join_body(lines: &[String]) -> String {
    let mut body = String::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(line);
        if body.chars().count() >= MAX_SECTION_BODY_CHARS {
            truncate_chars(&mut body, MAX_SECTION_BODY_CHARS);
            break;
        }
    }
    body
}

fn truncate_chars(text: &mut String, limit: usize) {
    if let Some((index, _)) = text.char_indices().nth(limit) {
        text.truncate(index);
    }
}

fn looks_like_contact(lines: &[String]) -> bool {
    lines.iter().any(|line| {
        line.contains('@')
            || line
                .split(|character: char| !character.is_ascii_digit())
                .any(|digits| digits.len() >= 7)
    })
}

/// Repair only known words, not arbitrary initials ("R programming", "A team",
/// "I enjoy") or names. Geometry handles inferred PDF spaces separately.
fn repair_initial(line: &str) -> String {
    let Some((initial, rest)) = line.split_once(' ') else {
        return line.to_owned();
    };
    if initial.chars().count() != 1 || !initial.chars().all(char::is_uppercase) {
        return line.to_owned();
    }
    let word = rest.split_whitespace().next().unwrap_or("");
    let joined = format!("{initial}{word}").to_lowercase();
    if [
        "experience",
        "education",
        "professional",
        "summary",
        "skills",
        "projects",
        "certifications",
        "languages",
        "interests",
        "employment",
        "engineer",
        "developed",
        "managed",
        "built",
        "designed",
        "implemented",
    ]
    .contains(&joined.as_str())
    {
        format!("{initial}{rest}")
    } else {
        line.to_owned()
    }
}

fn clean_lines(lines: &[String]) -> Vec<String> {
    let mut cleaned = Vec::new();
    for line in lines.iter().take(MAX_LINES) {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        let line = line.trim();
        if line.is_empty() || line.chars().all(char::is_numeric) {
            continue;
        }
        cleaned.push(line.to_owned());
    }
    cleaned
}

fn normalize(value: &str) -> String {
    let mut normalized = String::new();
    let value = value.replace('&', " and ");
    for character in value.chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
        } else if !normalized.ends_with(' ') {
            normalized.push(' ');
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn aliases_find_the_usual_headings_without_a_model() {
        let map = SectionDetector::heuristic().detect(&lines(&[
            "Ada Lovelace",
            "ada@example.com",
            "Experience",
            "Staff Engineer, Analytical Engines",
            "Built the first algorithm.",
            "Education",
            "BSc Mathematics",
            "Skills",
            "Rust, TypeScript, Postgres",
        ]));
        assert_eq!(map.engine, ENGINES);
        let kinds: Vec<SectionKind> = map.sections.iter().map(|section| section.kind).collect();
        assert_eq!(
            kinds,
            vec![
                SectionKind::Contact,
                SectionKind::Experience,
                SectionKind::Education,
                SectionKind::Skills,
            ]
        );
        assert_eq!(map.sections[1].heading, "Experience");
        assert!(map.sections[1].body.contains("Built the first algorithm."));
        assert_eq!(map.sections[1].id, "experience-1");
        assert_eq!(map.sections[3].id, "skills-1");
        assert!((map.sections[3].confidence - 0.97).abs() < f32::EPSILON);
    }

    #[test]
    fn headings_with_colons_and_caps_still_match() {
        let map = SectionDetector::heuristic().detect(&lines(&[
            "WORK EXPERIENCE:",
            "Did work.",
            "TECHNICAL SKILLS",
            "Rust",
        ]));
        assert_eq!(map.sections[0].kind, SectionKind::Experience);
        assert_eq!(map.sections[1].kind, SectionKind::Skills);
    }

    #[test]
    fn no_headings_still_produces_a_readable_section() {
        let map = SectionDetector::heuristic().detect(&lines(&["Ada Lovelace", "ada@example.com"]));
        assert_eq!(map.sections.len(), 1);
        assert_eq!(map.sections[0].kind, SectionKind::Contact);
        assert_eq!(map.sections[0].heading, "Contact");
    }

    #[test]
    fn body_prose_is_not_a_heading() {
        let map = SectionDetector::heuristic().detect(&lines(&[
            "Experience",
            "Built a very long sentence that is clearly not a heading because it has many words and punctuation.",
            "Shipped another thing.",
        ]));
        assert_eq!(map.sections.len(), 1);
        assert_eq!(map.sections[0].kind, SectionKind::Experience);
    }

    #[test]
    fn duplicate_adjacent_headings_collapse() {
        let map = SectionDetector::heuristic().detect(&lines(&[
            "Skills",
            "Rust",
            "Skills",
            "TypeScript",
        ]));
        assert_eq!(map.sections.len(), 1);
        assert_eq!(map.sections[0].kind, SectionKind::Skills);
        assert!(map.sections[0].body.contains("Rust"));
    }

    #[test]
    fn extraction_spacing_and_ampersands_match_known_labels() {
        let map = SectionDetector::heuristic().detect(&lines(&[
            "E X P E R I E N C E",
            "Built things.",
            "Edu cation",
            "BSc Mathematics",
            "Awards & Honors",
            "Scholarship",
        ]));
        assert_eq!(
            map.sections.iter().map(|s| s.kind).collect::<Vec<_>>(),
            vec![
                SectionKind::Experience,
                SectionKind::Education,
                SectionKind::Awards
            ]
        );
        assert_eq!(alias_kind("Experienced software engineer"), None);
    }

    #[test]
    fn opening_prose_is_not_automatically_contact_information() {
        let map = SectionDetector::heuristic().detect(&lines(&[
            "An experienced software engineer.",
            "Skills",
            "Rust",
        ]));
        assert_eq!(map.sections[0].kind, SectionKind::Other);
        assert_eq!(map.sections[0].body, "An experienced software engineer.");
    }

    #[test]
    fn learned_labels_need_independent_body_evidence() {
        assert!(!body_supports(
            SectionKind::Education,
            &lines(&["Built a compiler."])
        ));
        assert!(body_supports(
            SectionKind::Education,
            &lines(&["BSc at University of London"])
        ));
        // Do not borrow evidence from the next explicit section.
        assert!(!body_supports(
            SectionKind::Education,
            &lines(&["Skills", "University"])
        ));
        assert!(!body_supports(SectionKind::Skills, &[]));
    }

    #[test]
    fn sanity_check_repairs_known_words_not_initials_or_prose() {
        assert_eq!(repair_initial("E ngineer at Acme"), "Engineer at Acme");
        assert_eq!(
            repair_initial("P rofessional summary"),
            "Professional summary"
        );
        for text in [
            "A team player",
            "I enjoy programming",
            "R programming",
            "J Smith",
        ] {
            assert_eq!(repair_initial(text), text);
        }
    }

    #[test]
    fn formatting_is_support_not_a_label_or_permission_to_ignore_ambiguity() {
        let labels = vec![
            (SectionKind::Experience, vec![1., 0.]),
            (SectionKind::Education, vec![-1., 0.]),
        ];
        assert!(embedding_kind(&[0.6, 0.8], &labels, false).is_none());
        assert_eq!(
            embedding_kind(&[0.6, 0.8], &labels, true).unwrap().0,
            SectionKind::Experience
        );
        let ambiguous = vec![
            (SectionKind::Experience, vec![1., 0.]),
            (SectionKind::Education, vec![1., 0.]),
        ];
        assert!(embedding_kind(&[1., 0.], &ambiguous, true).is_none());
        let map = SectionDetector::heuristic().detect_document(&[
            domain::DocumentLine {
                text: "Acme Corporation".into(),
                emphasized: true,
                font_size: Some(24.),
            },
            domain::DocumentLine {
                text: "Built software.".into(),
                font_size: Some(12.),
                ..Default::default()
            },
        ]);
        assert_eq!(map.sections[0].kind, SectionKind::Other);
    }

    #[test]
    fn confidence_and_ids_round_trip_through_json() {
        let map = SectionDetector::heuristic().detect(&lines(&["Experience", "Built things."]));
        let json = serde_json::to_value(&map).unwrap();
        assert_eq!(json["sections"][0]["kind"], "experience");
        assert_eq!(json["sections"][0]["id"], "experience-1");
        let back: SectionMap = serde_json::from_value(json).unwrap();
        assert_eq!(back, map);
    }
}
