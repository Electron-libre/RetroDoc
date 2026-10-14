//! What the behaviours do not cover (ADR 0020), saved as `.retrodoc/cache/behaviour-coverage.yaml`
//! by a `generate` with `domains_from = "surface"`: the entry points no feature claimed, even after
//! the placement call (the uncategorized bucket of the entry points), and the source files reached
//! by no behaviour (technical support), and per feature the entry points no use case cites (the
//! use cases pass). The debt report reads it; it needs no LLM call.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::entry_points::EntryPoints;
use crate::error::PipelineError;
use crate::features::entry_point_id;
use crate::slices::CodeIndex;
use crate::surface::Surface;
use retrodoc_core::model::{Feature, UseCase};

/// As in the use cases pass: how far from an entry point's file the code it runs is followed.
const SLICE_DEPTH: usize = 2;
const MAX_SLICE_FILES: usize = 8;

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
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::BehaviourCoverage.path(repo_root), self)
    }
}

/// The source files no behaviour reaches: those of the entry points the `features` hold, the code
/// they run (a [`CodeIndex::slice`]) and the files that mention an entity of the surface are
/// reached; the others are technical support. Saved in the coverage artifact, next to the entry
/// points left unplaced by the features pass.
///
/// # Errors
///
/// Returns an error if the artifact can't be saved.
pub fn save_technical_support(
    repo_root: &Path,
    sources: &[PathBuf],
    entry_points: &EntryPoints,
    features: &[Feature],
    surface: &Surface,
) -> Result<BehaviourCoverage, PipelineError> {
    let documented: BTreeSet<&str> = features
        .iter()
        .flat_map(|f| f.entry_points.iter().map(String::as_str))
        .collect();
    let index = CodeIndex::new(sources.iter().map(PathBuf::as_path));
    let mut reached: BTreeSet<PathBuf> = BTreeSet::new();
    let mut starts: BTreeSet<PathBuf> = BTreeSet::new();
    for (file, entry) in entry_points.iter() {
        if documented.contains(entry_point_id(file, entry).as_str()) {
            starts.insert(file.to_path_buf());
        }
    }
    for file in &starts {
        reached.insert(file.clone());
        reached.extend(index.slice(
            repo_root,
            std::slice::from_ref(file),
            SLICE_DEPTH,
            MAX_SLICE_FILES,
        ));
    }
    for entity in &surface.entities {
        reached.extend(entity.files.iter().cloned());
    }
    let mut technical_support: Vec<PathBuf> = sources
        .iter()
        .filter(|f| !reached.contains(*f))
        .cloned()
        .collect();
    technical_support.sort();
    technical_support.dedup();

    let mut coverage = BehaviourCoverage::load(repo_root).unwrap_or_default();
    coverage.technical_support = technical_support;
    coverage.save(repo_root)?;
    Ok(coverage)
}

/// Which entry points of the `features` no use case of theirs cites, saved in the coverage artifact
/// (the other fields are kept). A feature without entry points of its own (built from files) is
/// not judged. Use cases saved before ids cite names: they cover nothing until derived again.
///
/// # Errors
///
/// Returns an error if the artifact can't be saved.
pub fn save_use_case_coverage(
    repo_root: &Path,
    features: &[Feature],
    use_cases: &[UseCase],
) -> Result<BehaviourCoverage, PipelineError> {
    let mut coverage = BehaviourCoverage::load(repo_root).unwrap_or_default();
    let before = coverage.by_feature.clone();
    for feature in features {
        coverage.by_feature.remove(&feature.slug);
        if feature.entry_points.is_empty() {
            continue;
        }
        let cited: BTreeSet<&str> = use_cases
            .iter()
            .filter(|u| u.feature_slug == feature.slug)
            .flat_map(|u| u.entry_points.iter().map(String::as_str))
            .collect();
        coverage.by_feature.insert(
            feature.slug.clone(),
            FeatureCoverage {
                entry_points: feature.entry_points.len(),
                uncovered: feature
                    .entry_points
                    .iter()
                    .filter(|id| !cited.contains(id.as_str()))
                    .cloned()
                    .collect(),
            },
        );
    }
    // A run from files has no entry point to judge: it leaves no artifact behind.
    if coverage.by_feature != before {
        coverage.save(repo_root)?;
    }
    Ok(coverage)
}

#[cfg(test)]
mod tests;
