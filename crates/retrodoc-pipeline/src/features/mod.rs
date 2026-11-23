//! Features pass (docs/ARCHITECTURE.md §4 step 4): the features of each domain, from its entry points
//! (`from_surface`, ADR 0020), saved as the intermediate `.retrodoc/cache/features.yaml` artifact, the
//! input of the use cases pass. The features per domain from the file summaries of the removed `files`
//! path (ADR 0035) are gone; this module keeps the artifact and the slugs.

use std::collections::BTreeSet;
use std::path::Path;

pub(crate) use retrodoc_core::artifacts::{load_features, load_technical_features, save_features};
use retrodoc_core::model::Feature;
use retrodoc_core::text::fold_latin;

use crate::artifact::{save_yaml, Artifact};
use crate::error::PipelineError;

mod from_surface;
pub use self::from_surface::{build_features_from_surface, entry_point_id};
pub(crate) use self::from_surface::{file_ranges, tag, tag_position};

/// The features of the last run of the features pass, those set aside as technical included.
fn load_surface_features(repo_root: &Path) -> Option<Vec<Feature>> {
    let mut features = load_features(repo_root)?;
    features.extend(load_technical_features(repo_root).unwrap_or_default());
    Some(features)
}

/// Saves `features` as the features pass leaves them: the technical ones in
/// `features-set-aside.yaml`, the others in `features.yaml`.
fn save_surface_features(repo_root: &Path, features: &[Feature]) -> Result<(), PipelineError> {
    let (technical, functional): (Vec<&Feature>, Vec<&Feature>) =
        features.iter().partition(|f| f.technical);
    // The set-aside first: a failure then leaves the previous `features.yaml`, whose technical
    // features a rerun would ask again, rather than technical ones lost to the placement.
    save_yaml(&Artifact::FeaturesSetAside.path(repo_root), &technical)?;
    save_yaml(&Artifact::Features.path(repo_root), &functional)
}

/// Lowercases `raw` into a kebab-case slug made of `[a-z0-9-]` only, safe
/// as a file name; an accented Latin letter keeps its base letter (DLG-5);
/// falls back to `"unnamed"` when nothing usable remains.
pub(crate) fn slugify(raw: &str) -> String {
    let mut slug = String::new();
    for c in fold_latin(raw).chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
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
