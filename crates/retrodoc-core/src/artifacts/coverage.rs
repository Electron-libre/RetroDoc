//! What the behaviours do not cover (ADR 0020), saved as `.retrodoc/cache/behaviour-coverage.yaml`:
//! the entry points no feature claimed, the source files reached by no behaviour (technical support),
//! and per domain and per feature the entry points left out. The pipeline computes it; the debt
//! report and the audit read it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cache::{load_yaml, save_yaml, Artifact, CacheError};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviourCoverage {
    /// Ids (`<file>#<name>`) of the entry points that belong to no feature.
    #[serde(default)]
    pub unplaced_entry_points: Vec<String>,
    /// Source files that hold no entry point of a feature, are not run by one and mention no entity.
    #[serde(default)]
    pub technical_support: Vec<PathBuf>,
    /// Per domain (bounded context) of a `domains_from = "contexts"` run: how many entry points it
    /// holds and how many of them no feature holds. The count of a domain is renewed when it runs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_domain: BTreeMap<String, EntryCount>,
    /// Per feature slug (features from the surface only): the entry points it holds and those no
    /// use case of it cites. Renewed for the features whose use cases were just derived.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_feature: BTreeMap<String, FeatureCoverage>,
}

/// The entry points of one feature and those its use cases leave out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureCoverage {
    pub entry_points: usize,
    /// Ids (`<file>#<name>`) cited by no use case of the feature.
    pub uncovered: Vec<String>,
}

/// The entry points of one domain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryCount {
    pub entry_points: usize,
    /// Held by no feature.
    pub unplaced: usize,
}

impl BehaviourCoverage {
    /// Missing or unreadable: `None` (first run, or a `files` run).
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::BehaviourCoverage.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the file can't be written or serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), CacheError> {
        save_yaml(&Artifact::BehaviourCoverage.path(repo_root), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_coverage_saved_before_the_counts_per_domain_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = Artifact::BehaviourCoverage.path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "unplaced_entry_points:\n- a.rb#GET /a\ntechnical_support: []\n",
        )
        .unwrap();

        let coverage = BehaviourCoverage::load(dir.path()).unwrap();

        assert_eq!(coverage.unplaced_entry_points, ["a.rb#GET /a"]);
        assert!(coverage.by_domain.is_empty() && coverage.by_feature.is_empty());
    }
}
