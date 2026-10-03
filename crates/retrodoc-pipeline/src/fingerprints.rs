//! Input fingerprints of the LLM-backed passes, the basis of the incremental
//! re-run (roadmap phase 6): a unit whose inputs hash to the same value as
//! on the last run keeps its previous result instead of being sent to the
//! LLM again. Stored in `.retrodoc/cache/fingerprints.json`.
//!
//! Each pass replaces its own map with the keys seen during the run, so
//! entries of vanished units are pruned. Changing the model or the prompts
//! does not invalidate anything: `retrodoc generate --force` wipes the caches.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::artifact::{load_json, save_json};
use crate::cache::hash_content;
use crate::error::PipelineError;

const FINGERPRINTS_RELATIVE_PATH: &str = ".retrodoc/cache/fingerprints.json";

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
}

impl Fingerprints {
    /// Missing or unreadable: empty (first run), not an error.
    pub fn load(repo_root: &Path) -> Self {
        load_json(&repo_root.join(FINGERPRINTS_RELATIVE_PATH)).unwrap_or_default()
    }

    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_json(&repo_root.join(FINGERPRINTS_RELATIVE_PATH), self)
    }
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
