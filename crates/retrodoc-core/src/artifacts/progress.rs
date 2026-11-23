//! How far the bounded contexts of the saved map got, as the audit page shows it (AUD-5), read without hashing a
//! file. The pipeline writes the two files it reads (`contexts.yaml` and `fingerprints.json`); this module reads
//! them through views of its own, so that the readers need not take the map and the fingerprints of the pipeline.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::cache::{load_json, load_yaml, Artifact};

/// The contexts of the saved map, split as `fingerprints.json` records the end of their chain: a
/// context whose files were edited since is still done until the next run of the pipeline judges it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecordedProgress {
    /// Slugs, in the order of the map.
    pub done: Vec<String>,
    pub pending: Vec<String>,
}

// The views type every field of the two files as the pipeline does (`ContextMap`, `Fingerprints`), the fields
// they don't read included: a file the pipeline can't read is none here either (`a_malformed_map_or_fingerprints_
// file_reads_the_same_in_core` in the pipeline).

/// `contexts.yaml`: the slugs of its contexts, in its order.
#[derive(Deserialize)]
struct MapView {
    contexts: Vec<ContextView>,
    #[serde(default, rename = "support")]
    _support: SupportView,
}

#[derive(Deserialize)]
struct ContextView {
    slug: String,
    #[serde(rename = "name")]
    _name: String,
    #[serde(default, rename = "description")]
    _description: String,
    #[serde(default, rename = "groups")]
    _groups: Vec<String>,
    #[serde(default, rename = "folders")]
    _folders: Vec<String>,
    #[serde(default, rename = "files")]
    _files: Vec<String>,
}

#[derive(Default, Deserialize)]
struct SupportView {
    #[serde(default, rename = "groups")]
    _groups: Vec<String>,
    #[serde(default, rename = "folders")]
    _folders: Vec<String>,
    #[serde(default, rename = "files")]
    _files: Vec<String>,
}

/// `fingerprints.json`: the contexts whose whole chain finished, the keys of its `contexts`.
#[derive(Default, Deserialize)]
struct FinishedView {
    #[serde(default)]
    contexts: BTreeMap<String, String>,
    #[serde(default, rename = "domains")]
    _domains: Option<String>,
    #[serde(default, rename = "features")]
    _features: BTreeMap<String, String>,
    #[serde(default, rename = "use_cases")]
    _use_cases: BTreeMap<String, String>,
    #[serde(default, rename = "rules")]
    _rules: BTreeMap<String, String>,
    #[serde(default, rename = "entry_point_placement")]
    _entry_point_placement: Option<String>,
    #[serde(default, rename = "entry_point_placements")]
    _entry_point_placements: BTreeMap<String, String>,
    #[serde(default, rename = "confidence_model")]
    _confidence_model: Option<String>,
}

/// The progress of the contexts of `contexts.yaml`. `None` when there is no map, or an empty one.
#[must_use]
pub fn recorded_progress(repo_root: &Path) -> Option<RecordedProgress> {
    let map = load_yaml::<MapView>(&Artifact::Contexts.path(repo_root))
        .filter(|m| !m.contexts.is_empty())?;
    let finished =
        load_json::<FinishedView>(&Artifact::Fingerprints.path(repo_root)).unwrap_or_default();
    let (done, pending) = map
        .contexts
        .into_iter()
        .map(|c| c.slug)
        .partition(|slug| finished.contexts.contains_key(slug));
    Some(RecordedProgress { done, pending })
}
