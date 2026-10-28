//! Features pass (PLAN.md §2 step 4): the features of each domain, from its entry points
//! (`from_surface`, ADR 0020), saved as the intermediate `.retrodoc/cache/features.yaml` artifact, the
//! input of the use cases pass. The features per domain from the file summaries of the removed `files`
//! path (ADR 0035) are gone; this module keeps the artifact, the slugs and the partial save.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::Feature;
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use crate::artifact::{load_yaml, save_yaml, warn_on_error, Artifact};
use crate::brief::{Evidence, ProductBrief};
use crate::domains::{DomainCluster, DomainMap};
use crate::entry_points::{EntryPoint, EntryPoints};
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, model_part, Fingerprints};
use crate::progress::Progress;
use crate::response::complete_json;

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

/// Key of a unit in the fingerprints and the progress line.
fn unit_key(domain_slug: &str, sub_slug: Option<&str>) -> String {
    format!("{domain_slug}/{}", sub_slug.unwrap_or("-"))
}

/// Best-effort save after a failure: the units not reached yet keep their
/// previous features and fingerprints (instead of being lost), so a rerun
/// resumes where this one stopped.
fn save_partial(
    repo_root: &Path,
    mut features: Vec<Feature>,
    previous: &[Feature],
    mut prints: Fingerprints,
    known_units: &BTreeMap<String, String>,
) {
    for old in previous {
        let reached = features
            .iter()
            .any(|f| f.domain_slug == old.domain_slug && f.sub_domain_slug == old.sub_domain_slug);
        let slug_taken = features.iter().any(|f| f.slug == old.slug);
        if reached || slug_taken {
            continue;
        }
        let unit_key = unit_key(&old.domain_slug, old.sub_domain_slug.as_deref());
        if let Some(print) = known_units.get(&unit_key) {
            prints.features.insert(unit_key, print.clone());
        }
        // The slices of a domain asked in several calls (`from_surface`).
        let slices = format!("{}/slice-", old.domain_slug);
        for (key, print) in known_units.iter().filter(|(k, _)| k.starts_with(&slices)) {
            prints.features.insert(key.clone(), print.clone());
        }
        features.push(old.clone());
    }
    warn_on_error(prints.save(repo_root));
    warn_on_error(save_features(repo_root, &features));
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
