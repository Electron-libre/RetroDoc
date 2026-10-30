//! Features pass (docs/ARCHITECTURE.md §4 step 4): the features of each domain, from its entry points
//! (`from_surface`, ADR 0020), saved as the intermediate `.retrodoc/cache/features.yaml` artifact, the
//! input of the use cases pass. The features per domain from the file summaries of the removed `files`
//! path (ADR 0035) are gone; this module keeps the artifact and the slugs.

use std::collections::BTreeSet;
use std::path::Path;

use retrodoc_core::model::Feature;

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::error::PipelineError;

mod from_surface;
pub use self::from_surface::{build_features_from_surface, entry_point_id};
pub(crate) use self::from_surface::{file_ranges, tag, tag_position};

/// Loads a previously saved `features.yaml`. Missing or unreadable: `None`
/// (first run), not an error.
#[must_use]
pub fn load_features(repo_root: &Path) -> Option<Vec<Feature>> {
    load_yaml(&Artifact::Features.path(repo_root))
}

/// Persists `features` as `.retrodoc/cache/features.yaml`; also used to
/// re-save once confidence scores are attached.
///
/// # Errors
///
/// Returns an error if the file can't be written or serialization fails.
pub fn save_features(repo_root: &Path, features: &[Feature]) -> Result<(), PipelineError> {
    save_yaml(&Artifact::Features.path(repo_root), features)
}

/// Lowercases `raw` into a kebab-case slug made of `[a-z0-9-]` only, safe
/// as a file name; falls back to `"unnamed"` when nothing usable remains.
pub(crate) fn slugify(raw: &str) -> String {
    let mut slug = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "unnamed".to_string()
    } else {
        slug.to_string()
    }
}

/// Slugifies `raw` and appends `-2`, `-3`… until it doesn't collide with
/// any of `taken`.
pub(crate) fn unique_slug<'a>(raw: &str, taken: impl Iterator<Item = &'a str>) -> String {
    let taken: BTreeSet<&str> = taken.collect();
    let base = slugify(raw);
    if !taken.contains(base.as_str()) {
        return base;
    }
    (2..=u32::MAX)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken.contains(candidate.as_str()))
        .expect("a free slug exists among u32::MAX candidates")
}

#[cfg(test)]
mod tests;
