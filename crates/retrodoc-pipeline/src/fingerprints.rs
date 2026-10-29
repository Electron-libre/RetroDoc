//! Input fingerprints of the LLM-backed passes, the basis of the incremental
//! re-run (roadmap phase 6): a unit whose inputs hash to the same value as
//! on the last run keeps its previous result instead of being sent to the
//! LLM again. Stored in `.retrodoc/cache/fingerprints.json`.
//!
//! Each pass replaces its own map with the keys seen during the run, so
//! entries of vanished units are pruned. The model of the pass is part of
//! what is hashed, so asking a pass for another model redoes it, and so are its
//! fixed instructions ([`prompt_part`], ADR 0037), so a changed prompt redoes
//! it once. `retrodoc generate --force` wipes the caches.

use std::collections::BTreeMap;
use std::path::Path;

use retrodoc_core::model::Feature;
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_json, save_json, Artifact};
use crate::cache::hash_content;
use crate::error::PipelineError;
use crate::walked::WalkedFiles;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Fingerprints {
    /// Domains pass: hash of the clustering input (file paths and summaries,
    /// existing docs). Equal to the last run's: `domains.yaml` is reused.
    #[serde(default)]
    pub domains: Option<String>,
    /// Features pass: `<domain>/-` (or `<domain>/slice-<n>`) → hash of the unit's files
    /// and their summaries.
    #[serde(default)]
    pub features: BTreeMap<String, String>,
    /// Use cases pass: `<domain>/<feature>` → hash of the feature's text and
    /// of the content of its files.
    #[serde(default)]
    pub use_cases: BTreeMap<String, String>,
    /// Rules pass: `<domain>/<feature>` → hash of the feature's text, code, evidence and use cases.
    #[serde(default)]
    pub rules: BTreeMap<String, String>,
    /// Features pass, surface mode: hash of what the placement of the entry points no feature claimed
    /// was asked with (`None`: nothing was left to place).
    #[serde(default)]
    pub entry_point_placement: Option<String>,
    /// The same, per domain, when the domains are documented a bounded context at a time (the
    /// placement of a context depends on that context alone).
    #[serde(default)]
    pub entry_point_placements: BTreeMap<String, String>,
    /// Contexts: slug → hash of the files of the context when its whole chain finished (the
    /// context is done while it still matches).
    #[serde(default)]
    pub contexts: BTreeMap<String, String>,
    /// Confidence pass: the model that scored the saved use cases. Another
    /// model scores them all again.
    #[serde(default)]
    pub confidence_model: Option<String>,
}

impl Fingerprints {
    /// Missing or unreadable: empty (first run), not an error.
    pub fn load(repo_root: &Path) -> Self {
        load_json(&Artifact::Fingerprints.path(repo_root)).unwrap_or_default()
    }

    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_json(&Artifact::Fingerprints.path(repo_root), self)
    }
}

/// The part of a fingerprint that says which model derived the result: a
/// pass asked to use another model redoes its units (ADR 0023).
pub(crate) fn model_part(llm: &dyn LlmProvider) -> String {
    format!("model {}", llm.model())
}

/// The part of a fingerprint that says which instructions derived the result: the system prompts and
/// the fixed text of the user prompts of the pass, constants of the binary, so a changed prompt redoes
/// the units of its pass once and two runs of the same binary compare equal (ADR 0037).
pub(crate) fn prompt_part(instructions: &[&str]) -> String {
    format!("prompt {}", fingerprint(instructions))
}

/// `hash` (of a unit of input) made dependent on the model of the pass.
pub(crate) fn hash_for_model(hash: &str, llm: &dyn LlmProvider) -> String {
    hash_content(&format!("{}\0{hash}", model_part(llm)))
}

/// The key of a feature in the prints of the passes that take a feature at a time.
pub(crate) fn feature_key(feature: &Feature) -> String {
    format!("{}/{}", feature.domain_slug, feature.slug)
}

/// The part of a print a cited file makes: its path and the hash of its content read through the
/// walk (an unreadable file hashes as such, so the print changes when it becomes readable).
pub(crate) fn file_part(walked: &WalkedFiles, path: &str) -> String {
    let hash = walked.read(Path::new(path)).map_or_else(
        |_| "unreadable".to_string(),
        |content| hash_content(&content),
    );
    format!("{path}\n{hash}")
}

/// Hash of `parts`, unambiguous regardless of how they are split.
pub(crate) fn fingerprint<S: AsRef<str>>(parts: impl IntoIterator<Item = S>) -> String {
    let mut joined = String::new();
    for part in parts {
        joined.push_str(part.as_ref());
        joined.push('\0');
    }
    hash_content(&joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_depends_on_part_boundaries() {
        assert_ne!(fingerprint(["ab", "c"]), fingerprint(["a", "bc"]));
        assert_eq!(fingerprint(["a", "b"]), fingerprint(["a", "b"]));
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut prints = Fingerprints::load(dir.path());
        prints.features.insert("a/-".to_string(), "h".to_string());
        prints.save(dir.path()).unwrap();
        assert_eq!(
            Fingerprints::load(dir.path()).features.get("a/-"),
            Some(&"h".to_string())
        );
    }
}
