//! Input fingerprints of the LLM-backed passes, the basis of the incremental
//! re-run (roadmap phase 6): a unit whose inputs hash to the same value as
//! on the last run keeps its previous result instead of being sent to the
//! LLM again. Stored in `.retrodoc/cache/fingerprints.json`.
//!
//! Each pass replaces its own map with the keys seen during the run, so
//! entries of vanished units are pruned. The model of the pass is part of
//! what is hashed, so asking a pass for another model redoes it; changing the
//! prompts does not invalidate anything: `retrodoc generate --force` wipes
//! the caches.

use std::collections::BTreeMap;
use std::path::Path;

use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_json, save_json, Artifact};
use crate::cache::hash_content;
use crate::error::PipelineError;

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Fingerprints {
    /// Domains pass: hash of the clustering input (file paths and summaries,
    /// existing docs). Equal to the last run's: `domains.yaml` is reused.
    #[serde(default)]
    pub domains: Option<String>,
    /// Features pass: `<domain>/<sub-domain or ->` → hash of the unit's files
    /// and their summaries.
    #[serde(default)]
    pub features: BTreeMap<String, String>,
    /// Use cases pass: `<domain>/<feature>` → hash of the feature's text and
    /// of the content of its files.
    #[serde(default)]
    pub use_cases: BTreeMap<String, String>,
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

/// `hash` (of a unit of input) made dependent on the model of the pass.
pub(crate) fn hash_for_model(hash: &str, llm: &dyn LlmProvider) -> String {
    hash_content(&format!("{}\0{hash}", model_part(llm)))
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
