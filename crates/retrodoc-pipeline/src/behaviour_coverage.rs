//! What the behaviours do not cover (ADR 0020), saved as `.retrodoc/cache/behaviour-coverage.yaml`
//! by a `generate` with `domains_from = "surface"`: the entry points no feature claimed, even after
//! the placement call (the uncategorized bucket of the entry points), and the source files reached
//! by no behaviour (technical support). The debt report reads it; it needs no LLM call.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::entry_points::EntryPoints;
use crate::error::PipelineError;
use crate::features::entry_point_id;
use crate::slices::CodeIndex;
use crate::surface::Surface;
use retrodoc_core::model::Feature;

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

#[cfg(test)]
mod tests;
