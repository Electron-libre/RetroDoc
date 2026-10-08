//! The product brief (ADR 0019): what the application does, for whom, its
//! main business objects and capabilities, written once from the non-code
//! evidence (docs, commits, manifests, schema, translations, tests...) so
//! that every later pass starts from a global frame.
//!
//! One LLM call over a bounded sample of the signals (`sample.rs`); each
//! claim cites the ids of the signals it rests on, which are resolved to
//! their origins (a citation that isn't in the sample is dropped, a claim
//! left with none is kept and reads as unsupported). The brief is saved as
//! `.retrodoc/cache/product.yaml`, hand-editable: a file whose content was
//! changed by hand is kept as is, an untouched one is reused while the
//! sample is the same (commits left out: they come all the time).

mod sample;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_ingest::signals::Signal;
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::cache::hash_content;
use crate::error::PipelineError;
use crate::response::complete_json;
use sample::{Sample, SAMPLE_BUDGET};

/// Entries kept per list of the brief.
const MAX_USERS: usize = 12;
const MAX_OBJECTS: usize = 25;
const MAX_CAPABILITIES: usize = 30;
const MAX_EXTERNAL: usize = 12;
const MAX_QUESTIONS: usize = 10;
/// Characters kept of a claim, and of the purpose (cut at a word).
const MAX_CLAIM_CHARS: usize = 300;
const MAX_PURPOSE_CHARS: usize = 600;

const SYSTEM_PROMPT: &str = "You are writing the product brief of a software application, from \
evidence that is not its code: documentation, commit subjects, manifests, database schema, texts \
shown to users, test descriptions and behaviour scenarios. Each piece of evidence has an id like \
[S12]. Write in business language, for someone who has never seen the code: say what the \
application is for and what people can do with it, not how it is built. Every claim must rest on \
evidence: cite at least one id in `signals`, and leave out a claim you cannot cite. Never invent: \
what the evidence does not settle goes to `open_questions`. Every entry of `users`, `objects`, \
`capabilities` and `external_systems` is an object with `text` and `signals`, never a bare \
string. Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching this \
shape: {\"purpose\":{\"text\":\"one or two sentences\",\"signals\":[\"S1\"]},\
\"users\":[{\"text\":\"who uses it or acts on it, with their goal\",\"signals\":[\"S2\"]}],\
\"objects\":[{\"text\":\"a main business object and what it stands for\",\"signals\":[\"S3\"]}],\
\"capabilities\":[{\"text\":\"something users can do, as a verb phrase\",\"signals\":[\"S4\",\"S7\"]}],\
\"external_systems\":[{\"text\":\"a third-party service it talks to\",\"signals\":[\"S5\"]}],\
\"open_questions\":[\"what the evidence leaves unclear\"]}. Leave a list empty rather than guess.";

/// One statement of the brief and the signals it rests on (their origins,
/// e.g. `README.md#Features` or `commit:ab12cd34`). No origin: unsupported.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub text: String,
    #[serde(default)]
    pub sources: Vec<String>,
}

impl Claim {
    #[must_use]
    pub fn is_supported(&self) -> bool {
        !self.sources.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductBrief {
    /// Hash of the sample the brief was written from (commits left out).
    #[serde(default)]
    pub input_hash: String,
    /// Hash of the content as written: a different hash on load means the
    /// file was edited by hand.
    #[serde(default)]
    pub content_hash: String,
    #[serde(default)]
    pub purpose: Claim,
    /// Who uses the application or acts on it.
    #[serde(default)]
    pub users: Vec<Claim>,
    /// The main business objects.
    #[serde(default)]
    pub objects: Vec<Claim>,
    /// What the application lets people do.
    #[serde(default)]
    pub capabilities: Vec<Claim>,
    #[serde(default)]
    pub external_systems: Vec<Claim>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

impl ProductBrief {
    /// Loads `.retrodoc/cache/product.yaml`. Missing or unreadable: `None`.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::Product.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::Product.path(repo_root), self)
    }

    /// Whether the content differs from what was written (a hand edit).
    #[must_use]
    pub fn edited(&self) -> bool {
        self.digest() != self.content_hash
    }

    fn digest(&self) -> String {
        let mut content = self.clone();
        content.input_hash.clear();
        content.content_hash.clear();
        hash_content(&serde_yaml::to_string(&content).unwrap_or_default())
    }

    /// The brief as the head of a prompt: the words only, no citations, so
    /// the later passes start from the same global frame. Empty for an empty
    /// brief.
    #[must_use]
    pub fn prompt_section(&self) -> String {
        let mut out = String::new();
        if !self.purpose.text.is_empty() {
            let _ = writeln!(out, "Purpose: {}", self.purpose.text);
        }
        for (title, claims) in [
            ("Users and actors", &self.users),
            ("Main business objects", &self.objects),
            ("Capabilities", &self.capabilities),
            ("External systems", &self.external_systems),
        ] {
            if claims.is_empty() {
                continue;
            }
            let _ = writeln!(out, "{title}:");
            for claim in claims {
                let _ = writeln!(out, "- {}", claim.text);
            }
        }
        if out.is_empty() {
            return out;
        }
        format!(
            "Product brief of the application (what it is for, from its docs and history):\n{out}"
        )
    }

    /// The section followed by a blank line, to put at the head of a prompt;
    /// empty for an empty brief.
    #[must_use]
    pub fn prompt_head(&self) -> String {
        let section = self.prompt_section();
        if section.is_empty() {
            section
        } else {
            format!("{section}\n")
        }
    }

    /// Hash of a unit of input (a file) as read under this brief: the plain
    /// hash of the content for an empty brief, so caches written before the
    /// brief existed stay valid; otherwise it also depends on the brief.
    #[must_use]
    pub fn hash_with(&self, content: &str) -> String {
        let fingerprint = self.fingerprint();
        if fingerprint.is_empty() {
            hash_content(content)
        } else {
            hash_content(&format!("{fingerprint}\0{content}"))
        }
    }

    /// What the passes that read the brief put in their own fingerprint, so
    /// that editing it redoes them. Empty for an empty brief.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let section = self.prompt_section();
        if section.is_empty() {
            section
        } else {
            hash_content(&section)
        }
    }
}

/// Characters of evidence the LLM would be sent for these signals.
#[must_use]
pub fn sample_chars(signals: &[Signal]) -> usize {
    Sample::build(signals, SAMPLE_BUDGET).text.chars().count()
}

/// Writes (or reuses) the product brief from the signals of the repository.
/// Returns `None` when there is no signal at all, or when the LLM gave no
/// parseable answer (a warning says so; nothing is saved). `force` writes it
/// again whatever the file holds, hand edits included (as `roles --force`).
/// A saved file that can't be read is left alone.
///
/// # Errors
///
/// Returns an error if the LLM call fails or the brief can't be saved.
pub async fn build_brief(
    repo_root: &Path,
    signals: &[Signal],
    llm: &dyn LlmProvider,
    force: bool,
) -> Result<Option<ProductBrief>, PipelineError> {
    let sample = Sample::build(signals, SAMPLE_BUDGET);
    if sample.origins.is_empty() {
        tracing::info!("no signal to write a product brief from");
        return Ok(None);
    }
    let input_hash = hash_content(&sample.text_without_commits());

    if !force {
        if Artifact::Product.path(repo_root).exists() && ProductBrief::load(repo_root).is_none() {
            tracing::warn!(
                "{} can't be read, no product brief used; fix it or run `retrodoc brief --force`",
                Artifact::Product.relative_path()
            );
            return Ok(None);
        }
        if let Some(saved) = ProductBrief::load(repo_root) {
            if saved.edited() {
                tracing::info!("using the hand-edited product brief");
                return Ok(Some(saved));
            }
            if saved.input_hash == input_hash {
                tracing::info!("product brief unchanged, reused");
                return Ok(Some(saved));
            }
        }
    }

    let prompt = format!("Evidence:\n\n{}", sample.text);
    let Some(raw) = complete_json::<RawBrief>(llm, SYSTEM_PROMPT, &prompt, "product brief").await?
    else {
        return Ok(None);
    };
    let mut brief = raw.resolve(&sample);
    brief.input_hash = input_hash;
    brief.content_hash = brief.digest();
    brief.save(repo_root)?;
    Ok(Some(brief))
}

/// A claim as the LLM writes it: a bare string or `{text, signals}`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawClaim {
    Text(String),
    Full {
        #[serde(default)]
        text: String,
        #[serde(default)]
        signals: Vec<String>,
    },
}

#[derive(Debug, Default, Deserialize)]
struct RawBrief {
    #[serde(default)]
    purpose: Option<RawClaim>,
    #[serde(default)]
    users: Vec<RawClaim>,
    #[serde(default)]
    objects: Vec<RawClaim>,
    #[serde(default)]
    capabilities: Vec<RawClaim>,
    #[serde(default)]
    external_systems: Vec<RawClaim>,
    #[serde(default)]
    open_questions: Vec<String>,
}

impl RawBrief {
    fn resolve(self, sample: &Sample) -> ProductBrief {
        ProductBrief {
            input_hash: String::new(),
            content_hash: String::new(),
            purpose: self
                .purpose
                .and_then(|c| resolve_claim(c, sample, MAX_PURPOSE_CHARS))
                .unwrap_or_default(),
            users: resolve_claims(self.users, sample, MAX_USERS),
            objects: resolve_claims(self.objects, sample, MAX_OBJECTS),
            capabilities: resolve_claims(self.capabilities, sample, MAX_CAPABILITIES),
            external_systems: resolve_claims(self.external_systems, sample, MAX_EXTERNAL),
            open_questions: self
                .open_questions
                .into_iter()
                .map(|q| q.trim().to_string())
                .filter(|q| !q.is_empty())
                .take(MAX_QUESTIONS)
                .collect(),
        }
    }
}

fn resolve_claims(raw: Vec<RawClaim>, sample: &Sample, max: usize) -> Vec<Claim> {
    let mut seen = BTreeSet::new();
    raw.into_iter()
        .filter_map(|claim| resolve_claim(claim, sample, MAX_CLAIM_CHARS))
        .filter(|claim| seen.insert(claim.text.to_lowercase()))
        .take(max)
        .collect()
}

/// The claim with its citations resolved to origins (unknown ids dropped,
/// the same origin once). `None` for an empty text.
fn resolve_claim(raw: RawClaim, sample: &Sample, max_chars: usize) -> Option<Claim> {
    let (text, ids) = match raw {
        RawClaim::Text(text) => (text, Vec::new()),
        RawClaim::Full { text, signals } => (text, signals),
    };
    let text = cut_at_word(
        &text.split_whitespace().collect::<Vec<_>>().join(" "),
        max_chars,
    );
    if text.is_empty() {
        return None;
    }
    let mut sources = Vec::new();
    for id in ids {
        let id = id.trim().trim_matches(['[', ']']).to_uppercase();
        match sample.origins.get(&id) {
            Some(origin) if !sources.contains(origin) => sources.push(origin.clone()),
            Some(_) => {}
            None => tracing::debug!(id, "the brief cites a signal that isn't in the sample"),
        }
    }
    Some(Claim { text, sources })
}

/// `text` cut before a word within `max` characters, `…` marking the cut.
fn cut_at_word(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{}…", cut.trim_end_matches([',', ';', ':', ' ']))
}

#[cfg(test)]
mod tests;
