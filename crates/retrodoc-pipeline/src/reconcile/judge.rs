//! The `reconcile` pass: the LLM tells, for each section matched to an item, whether it covers the item,
//! contradicts it or only shares its words. Items go [`MAX_ITEMS_PER_REQUEST`] at a time, the sections of
//! a request listed once. A verdict is saved with the fingerprint of the item's text and of its sections'
//! text, and the model: an unchanged item makes no call, and the report (which has no model) still reads a
//! verdict while the texts match. A section left unanswered keeps its item out of the saved verdicts, so
//! the next run asks it again. A section is shown within its share of [`MAX_REQUEST_SECTION_CHARS`]: a
//! longer one by the paragraphs that hold the words of the items of the request it is matched to.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::config::PipelineConfig;
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use super::{Candidate, ItemKind, Section};
use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::bm25::Bm25;
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, model_part, prompt_part};
use crate::response::complete_json;
use crate::text::{char_prefix, without_last_chars};

/// Items judged in one request.
pub const MAX_ITEMS_PER_REQUEST: usize = 8;

/// Characters of section text a request shows at most, shared evenly between its sections.
const MAX_REQUEST_SECTION_CHARS: usize = 36_000;

/// The share of a section is never smaller.
const MIN_SECTION_CHARS: usize = 1_500;

/// Names the rule that picks what is shown of a section: part of a verdict's fingerprint, so that a
/// change of the rule asks the saved items again. Change it when [`shown`] changes.
pub(super) const SHOWN_RULE: &str = "paragraphs-of-the-items-v3";

/// Marks the paragraphs of a section left out.
const LEFT_OUT: &str = "[...]";

const SYSTEM_PROMPT: &str = "You compare the functional documentation generated from the code of an \
application with the project's own documents. You are given sections of the existing documents, \
labelled S1, S2, ..., then numbered items of the generated documentation (a domain, a feature, a use case \
or a business rule), each with the sections that seem to speak of it. For each section of each item, \
answer one of:\n\
covers: the section describes the item, and nothing it says is incompatible with the item.\n\
contradicts: the section describes the same thing but states something incompatible with the item: \
another limit, amount, delay, order of steps, actor or outcome. A document may be out of date: still \
answer contradicts.\n\
unrelated: the section only shares words with the item, it is about something else.\n\
For contradicts, also write doc_states (what the section states, one sentence) and item_states (what the \
item states instead, one sentence). Reply with ONLY a single JSON object, no prose and no Markdown code \
fence: {\"items\":[{\"n\":1,\"sections\":[{\"s\":\"S1\",\"answer\":\"covers\"},{\"s\":\"S2\",\
\"answer\":\"contradicts\",\"doc_states\":\"...\",\"item_states\":\"...\"}]}]}.";

/// What a section is to an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Answer {
    Covers,
    Contradicts,
    Unrelated,
}

/// The answer on one section of an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgedSection {
    /// `path#heading`.
    pub section: String,
    pub answer: Answer,
    /// With `contradicts`: what the section states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_states: Option<String>,
    /// With `contradicts`: what the generated item states instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_states: Option<String>,
}

/// The verdict on one item, every section answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Judged {
    pub kind: ItemKind,
    pub label: String,
    /// Of the item's text and of its sections' text ([`Candidate::fingerprint`]).
    pub fingerprint: String,
    /// The model that answered.
    pub model: String,
    pub sections: Vec<JudgedSection>,
}

/// `.retrodoc/cache/reconciliation.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Judgements {
    #[serde(default)]
    pub items: Vec<Judged>,
}

impl Judgements {
    /// Missing or unreadable: none.
    #[must_use]
    pub fn load(repo_root: &Path) -> Self {
        load_yaml(&Artifact::Reconciliation.path(repo_root)).unwrap_or_default()
    }

    /// # Errors
    ///
    /// Returns an error if the file can't be written.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::Reconciliation.path(repo_root), self)
    }

    /// The verdict on `candidate` while its texts are the ones it was given on.
    #[must_use]
    pub fn of(&self, candidate: &Candidate) -> Option<&Judged> {
        let print = candidate.fingerprint();
        self.items.iter().find(|j| {
            j.kind == candidate.item.kind
                && j.label == candidate.item.label
                && j.fingerprint == print
        })
    }
}

impl Candidate {
    /// Hash of the item's text, of the origin and text of its sections, of [`SHOWN_RULE`] and of the
    /// system prompt (ADR 0037).
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let mut parts = vec![
            SHOWN_RULE.to_string(),
            prompt_part(&[SYSTEM_PROMPT]),
            self.item.kind.label().to_string(),
            self.item.query.clone(),
        ];
        for section in &self.sections {
            parts.push(section.at.origin());
            parts.push(section.text.clone());
        }
        fingerprint(parts)
    }

    /// Whether the LLM is asked about it: it has sections and cites none itself.
    fn to_judge(&self) -> bool {
        !self.sections.is_empty() && !self.cites_its_docs()
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawAnswer {
    #[serde(default)]
    items: Vec<RawItem>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawItem {
    /// Number of the item in the prompt.
    #[serde(default)]
    #[schemars(with = "u32")]
    n: serde_json::Value,
    #[serde(default)]
    sections: Vec<RawSection>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawSection {
    /// Label of the section, `S1`.
    #[serde(default)]
    #[schemars(with = "String")]
    s: serde_json::Value,
    #[serde(default)]
    answer: String,
    #[serde(default)]
    doc_states: Option<String>,
    #[serde(default)]
    item_states: Option<String>,
}

/// A number written as a number, `"2"`, `"S2"` or `2.0`.
fn number(value: &serde_json::Value) -> Option<usize> {
    let text = match value {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        _ => return None,
    };
    text.trim()
        .trim_start_matches(['S', 's'])
        .trim_end_matches(".0")
        .parse()
        .ok()
}

/// `text` on one line, so that it can't forge a numbered line of the prompt.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The sections of `batch` listed once, in the order they first appear: origin → label number.
fn section_numbers(batch: &[&Candidate]) -> BTreeMap<String, usize> {
    let mut numbers = BTreeMap::new();
    for section in batch.iter().flat_map(|c| &c.sections) {
        let next = numbers.len() + 1;
        numbers.entry(section.at.origin()).or_insert(next);
    }
    numbers
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

/// `text` cut to `n` chars at most, a code fence it leaves open closed.
fn cut(text: &str, n: usize) -> String {
    let Some(head) = char_prefix(text, n) else {
        return text.to_string();
    };
    let mut out = without_last_chars(head, 4).trim_end().to_string();
    if out.lines().filter(|l| is_fence(l)).count() % 2 == 1 {
        out.push_str("\n```");
    }
    out
}

/// The paragraphs of `body`: split on lines that are blank, but not inside a fenced code block.
fn paragraphs(body: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut in_fence = false;
    for line in body.lines() {
        if is_fence(line) {
            in_fence = !in_fence;
        }
        if line.trim().is_empty() && !in_fence {
            if !current.is_empty() {
                paragraphs.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join("\n"));
    }
    paragraphs
}

/// What is shown of `section` within `share` chars: the whole text when it fits, else its heading line
/// and the paragraphs [`Bm25`] ranks against each of `queries`, best first for each query in turn, each
/// cut to the share of one query, shown in the section's order with [`LEFT_OUT`] where paragraphs are
/// left out.
fn shown(section: &Section, queries: &[&str], share: usize) -> String {
    let text = section.text.trim();
    if char_prefix(text, share).is_none() {
        return text.to_string();
    }
    // The text before the first heading has no heading line.
    let (heading, body) = match section.at.heading {
        Some(_) => text.split_once('\n').unwrap_or((text, "")),
        None => ("", text),
    };
    let paragraphs = paragraphs(body);
    let index = Bm25::new(&paragraphs);
    let mut ranked: Vec<std::vec::IntoIter<(usize, f32)>> = queries
        .iter()
        .map(|query| index.search_any(query).into_iter())
        .collect();
    // A kept paragraph comes with a separator and a [...] line at most; the last [...] is set aside.
    let overhead = LEFT_OUT.len() + 4;
    let mut left = share.saturating_sub(heading.chars().count() + LEFT_OUT.len() + 2);
    let per_query = (left / queries.len().max(1)).saturating_sub(overhead);
    // Paragraph index → the text kept of it.
    let mut kept: BTreeMap<usize, String> = BTreeMap::new();
    let mut taking = true;
    while taking {
        taking = false;
        for paragraphs_of_query in &mut ranked {
            for (i, _) in paragraphs_of_query.by_ref() {
                let paragraph = cut(&paragraphs[i], per_query);
                let cost = paragraph.chars().count() + overhead;
                if kept.contains_key(&i) || cost > left {
                    continue;
                }
                kept.insert(i, paragraph);
                left -= cost;
                taking = true;
                break;
            }
        }
    }
    if kept.is_empty() {
        return cut(text, share);
    }
    let mut out = heading.to_string();
    let mut next = 0;
    for (i, paragraph) in kept {
        if i > next {
            out.push_str("\n\n");
            out.push_str(LEFT_OUT);
        }
        out.push_str("\n\n");
        out.push_str(&paragraph);
        next = i + 1;
    }
    if next < paragraphs.len() {
        out.push_str("\n\n");
        out.push_str(LEFT_OUT);
    }
    out.trim_start().to_string()
}

/// The chars each of `lengths` (of the sections of a request) may show: a section within an even share of
/// what is left is shown whole, the rest shared evenly by the longer ones, never under [`MIN_SECTION_CHARS`].
fn shares(lengths: &[usize]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..lengths.len()).collect();
    order.sort_by_key(|&i| lengths[i]);
    let mut shares = vec![0; lengths.len()];
    let mut left = MAX_REQUEST_SECTION_CHARS;
    for (done, &i) in order.iter().enumerate() {
        let even = (left / (lengths.len() - done)).max(MIN_SECTION_CHARS);
        shares[i] = lengths[i].min(even);
        left = left.saturating_sub(shares[i]);
    }
    shares
}

fn prompt(batch: &[&Candidate], numbers: &BTreeMap<String, usize>) -> String {
    let mut by_number: Vec<(usize, &Section)> = Vec::new();
    for section in batch.iter().flat_map(|c| &c.sections) {
        let n = numbers[&section.at.origin()];
        if !by_number.iter().any(|(m, _)| *m == n) {
            by_number.push((n, section));
        }
    }
    let lengths: Vec<usize> = by_number
        .iter()
        .map(|(_, s)| s.text.trim().chars().count())
        .collect();
    let shares = shares(&lengths);
    let mut prompt = String::from("Sections of the existing documents:\n");
    for ((n, section), share) in by_number.into_iter().zip(shares) {
        let origin = section.at.origin();
        let queries: Vec<&str> = batch
            .iter()
            .filter(|c| c.sections.iter().any(|s| s.at.origin() == origin))
            .map(|c| c.item.query.as_str())
            .collect();
        let _ = write!(
            prompt,
            "\n[S{n}] {origin}\n{}\n",
            shown(section, &queries, share)
        );
    }
    prompt.push_str("\nItems of the generated documentation:\n");
    for (i, candidate) in batch.iter().enumerate() {
        let labels: Vec<String> = candidate
            .sections
            .iter()
            .map(|s| format!("S{}", numbers[&s.at.origin()]))
            .collect();
        let _ = writeln!(
            prompt,
            "{}. {}: {} (sections: {})",
            i + 1,
            candidate.item.kind.label(),
            one_line(&candidate.item.query),
            labels.join(", ")
        );
    }
    prompt
}

/// The verdicts `answer` gives the items of `batch`: only the items whose every section got a valid
/// answer (a contradiction says what both sides state).
fn read_answer(
    batch: &[&Candidate],
    numbers: &BTreeMap<String, usize>,
    answer: &RawAnswer,
    model: &str,
) -> Vec<Judged> {
    let mut judged = Vec::new();
    for (i, candidate) in batch.iter().enumerate() {
        let Some(raw) = answer.items.iter().find(|r| number(&r.n) == Some(i + 1)) else {
            continue;
        };
        let sections: Option<Vec<JudgedSection>> = candidate
            .sections
            .iter()
            .map(|section| {
                let origin = section.at.origin();
                let wanted = numbers[&origin];
                let raw = raw.sections.iter().find(|s| number(&s.s) == Some(wanted))?;
                let answer = match raw.answer.trim().to_lowercase().as_str() {
                    "covers" => Answer::Covers,
                    "unrelated" => Answer::Unrelated,
                    "contradicts" => Answer::Contradicts,
                    _ => return None,
                };
                let said =
                    |text: &Option<String>| text.as_deref().map(one_line).filter(|t| !t.is_empty());
                let (doc_states, item_states) = if answer == Answer::Contradicts {
                    (Some(said(&raw.doc_states)?), Some(said(&raw.item_states)?))
                } else {
                    (None, None)
                };
                Some(JudgedSection {
                    section: origin,
                    answer,
                    doc_states,
                    item_states,
                })
            })
            .collect();
        if let Some(sections) = sections {
            judged.push(Judged {
                kind: candidate.item.kind,
                label: candidate.item.label.clone(),
                fingerprint: candidate.fingerprint(),
                model: model.to_string(),
                sections,
            });
        }
    }
    judged
}

/// Judges the matched items of `candidates` not judged yet with the same texts and model, and saves
/// the verdicts of every candidate (the others are dropped). Nothing when `[pipeline] reconcile` is off.
///
/// An LLM call that fails ends the pass with a warning, the verdicts got before saved.
///
/// # Errors
///
/// Returns an error if the verdicts can't be saved.
pub async fn judge(
    repo_root: &Path,
    llm: &dyn LlmProvider,
    config: &PipelineConfig,
    candidates: &[Candidate],
) -> Result<(), PipelineError> {
    if !config.reconcile {
        return Ok(());
    }
    let model = model_part(llm);
    let saved = Judgements::load(repo_root);
    let mut kept = Judgements::default();
    let mut to_ask: Vec<&Candidate> = Vec::new();
    for candidate in candidates.iter().filter(|c| c.to_judge()) {
        match saved.of(candidate).filter(|j| j.model == model) {
            Some(judged) => kept.items.push(judged.clone()),
            None => to_ask.push(candidate),
        }
    }
    if to_ask.is_empty() {
        if kept != saved {
            kept.save(repo_root)?;
        }
        return Ok(());
    }
    tracing::info!(
        items = to_ask.len(),
        reused = kept.items.len(),
        "judging the doc sections matched to the items"
    );
    for batch in to_ask.chunks(MAX_ITEMS_PER_REQUEST) {
        let numbers = section_numbers(batch);
        let asked = complete_json::<RawAnswer>(
            llm,
            SYSTEM_PROMPT,
            &prompt(batch, &numbers),
            "reconciliation with the existing docs",
        )
        .await;
        let answer = match asked {
            Ok(answer) => answer,
            Err(err) => {
                // The pass only feeds the report: the items left stay matched, asked next run.
                tracing::warn!(
                    "reconciliation with the existing docs stopped, the items left are asked next \
                     run: {err}"
                );
                break;
            }
        };
        if let Some(answer) = answer {
            let judged = read_answer(batch, &numbers, &answer, &model);
            if judged.len() < batch.len() {
                tracing::info!(
                    unanswered = batch.len() - judged.len(),
                    "items left to the next run: a section got no valid answer"
                );
            }
            kept.items.extend(judged);
        }
        kept.save(repo_root)?;
    }
    Ok(())
}
