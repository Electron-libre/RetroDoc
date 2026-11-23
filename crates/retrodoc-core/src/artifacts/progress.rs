//! How far the bounded contexts of the saved map got, as the audit page shows it (AUD-5), read without hashing a
//! file. The pipeline writes the two files it reads (`contexts.yaml` and `fingerprints.json`); this module reads
//! only the part of each it needs, through views of its own, so that the readers need not know the whole map nor
//! the fingerprints of the passes.

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

/// The slugs of `contexts.yaml`, in its order.
#[derive(Deserialize)]
struct MapView {
    contexts: Vec<ContextView>,
}

#[derive(Deserialize)]
struct ContextView {
    slug: String,
}

/// The contexts whose whole chain finished, keys of `fingerprints.json`'s `contexts` (their hashes not read).
#[derive(Default, Deserialize)]
struct FinishedView {
    #[serde(default)]
    contexts: BTreeMap<String, serde_json::Value>,
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
