//! Features pass (PLAN.md §2 step 4, roadmap phase 4): for each domain (and
//! each of its sub-domains), one LLM call turns the file summaries of that
//! unit into a short list of user-facing features, each grounded on the
//! subset of the unit's files that implements it. Saved as the intermediate
//! `.retrodoc/cache/features.yaml` artifact, the input of the use cases pass.
//!
//! The "uncategorized" domain is skipped: its files were explicitly not
//! understood by the clustering pass, so inventing features for them would
//! only produce noise.
//!
//! A malformed LLM answer for one unit is logged and that unit skipped
//! rather than aborting the whole run (there can be hundreds of units);
//! transport-level LLM failures still propagate.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_core::model::Feature;
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use crate::artifact::{load_yaml, save_yaml, warn_on_error, Artifact};
use crate::brief::{Evidence, ProductBrief};
use crate::domains::{DomainCluster, DomainMap, UNCATEGORIZED_SLUG};
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, model_part, Fingerprints};
use crate::progress::Progress;
use crate::repo_map::{FileSummary, RepoMap};
use crate::response::complete_json;

/// Upper bound on files listed in one features prompt (PLAN.md §6
/// "cost/volume"). When a unit has more, the most-modified files are kept:
/// they are the most likely to carry user-facing behavior.
const MAX_FILES_PER_PROMPT: usize = 150;

const FEATURES_SYSTEM_PROMPT: &str = "You are documenting a software project from a functional \
(business) point of view. Given a functional domain (or sub-domain) and the role summaries of its \
source files, identify the user-facing features it provides: capabilities a user or an external \
system would recognize, not technical layers. Group the files implementing each feature. Every \
feature must list at least one file, copied verbatim from the list given to you; a file may \
support several features. Reply with ONLY a single JSON object, no prose and no Markdown code \
fence, matching this shape: {\"features\":[{\"slug\":\"kebab-case\",\"name\":\"...\",\
\"description\":\"one or two sentences\",\"files\":[\"...\"]}]}.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawFeatures {
    #[serde(default)]
    features: Vec<RawFeature>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawFeature {
    slug: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    files: Vec<PathBuf>,
}

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

/// Derives the features of every domain/sub-domain in `domains` and
/// persists them to `.retrodoc/cache/features.yaml`.
///
/// Incremental: a unit whose files and file summaries are unchanged since
/// the last run keeps the features saved then, without an LLM call. The
/// domain's name and description are deliberately not part of the
/// fingerprint, so a re-worded clustering doesn't invalidate everything.
///
/// # Errors
///
/// Returns an error if an LLM call fails or the artifact can't be saved.
pub async fn build_features(
    repo_root: &Path,
    domains: &DomainMap,
    repo_map: &RepoMap,
    brief: &ProductBrief,
    evidence: &Evidence,
    llm: &dyn LlmProvider,
) -> Result<Vec<Feature>, PipelineError> {
    let summaries: BTreeMap<&Path, &FileSummary> = repo_map
        .files
        .iter()
        .map(|f| (f.path.as_path(), f))
        .collect();

    let previous = load_features(repo_root).unwrap_or_default();
    let mut prints = Fingerprints::load(repo_root);
    let known_units = std::mem::take(&mut prints.features);

    let mut progress = Progress::new("features", units(domains).count());

    let mut features: Vec<Feature> = Vec::new();
    for unit in units(domains) {
        let unit_key = unit_key(&unit.domain.slug, unit.sub_slug);
        let close = evidence.section(&unit_query(&unit));
        let unit_print = fingerprint(
            unit.paths
                .iter()
                .map(|p| {
                    let summary = summaries.get(p.as_path()).map_or("", |f| &f.role_summary);
                    format!("{}\n{summary}", p.display())
                })
                .chain((!brief.fingerprint().is_empty()).then(|| brief.fingerprint()))
                .chain((!close.is_empty()).then(|| close.clone()))
                .chain(std::iter::once(model_part(llm))),
        );
        if known_units.get(&unit_key) == Some(&unit_print) {
            if let Some(kept) = reusable(&previous, &features, &unit) {
                tracing::info!(unit = %unit_key, "features unchanged, reused");
                features.extend(kept);
                prints.features.insert(unit_key, unit_print);
                progress.skip();
                continue;
            }
        }
        progress.begin(&unit_key);
        let unit_files: Vec<&FileSummary> = unit
            .paths
            .iter()
            .filter_map(|p| summaries.get(p.as_path()).copied())
            .collect();
        let prompt = features_prompt(
            brief,
            &unit.domain.name,
            &unit.domain.description,
            unit.sub_name,
            &close,
            &unit_files,
        );
        let what = format!("features of {unit_key}");
        let raw =
            match complete_json::<RawFeatures>(llm, FEATURES_SYSTEM_PROMPT, &prompt, &what).await {
                Ok(Some(raw)) => raw,
                Ok(None) => continue,
                Err(err) => {
                    // Keep the units done so far, so a rerun resumes here.
                    save_partial(repo_root, features, &previous, prints, &known_units);
                    return Err(err);
                }
            };

        let produced = validated_features(raw, &unit, &features);
        if !produced.is_empty() {
            prints.features.insert(unit_key, unit_print);
        }
        features.extend(produced);
    }

    prints.save(repo_root)?;
    save_features(repo_root, &features)?;
    Ok(features)
}

/// A domain, or one of its sub-domains, as the unit of work of the pass.
struct Unit<'a> {
    domain: &'a DomainCluster,
    sub_slug: Option<&'a str>,
    sub_name: &'a str,
    paths: &'a [PathBuf],
}

/// The units to go through: each domain's own files, then each sub-domain's
/// (the "uncategorized" domain and empty units are skipped).
fn units(domains: &DomainMap) -> impl Iterator<Item = Unit<'_>> {
    domains
        .domains
        .iter()
        .filter(|d| d.slug != UNCATEGORIZED_SLUG)
        .flat_map(|domain| {
            let own = Unit {
                domain,
                sub_slug: None,
                sub_name: "",
                paths: &domain.paths,
            };
            let subs = domain.sub_domains.iter().map(move |sub| Unit {
                domain,
                sub_slug: Some(sub.slug.as_str()),
                sub_name: sub.name.as_str(),
                paths: &sub.paths,
            });
            std::iter::once(own).chain(subs)
        })
        .filter(|unit| !unit.paths.is_empty())
}

/// What the evidence closest to a unit is searched with: its names and
/// description, and its files (without their extension).
fn unit_query(unit: &Unit<'_>) -> String {
    let mut query = format!(
        "{} {} {}",
        unit.domain.name, unit.domain.description, unit.sub_name
    );
    for path in unit.paths {
        let _ = write!(query, " {}", path.with_extension("").display());
    }
    query
}

/// Key of a unit in the fingerprints and the progress line.
fn unit_key(domain_slug: &str, sub_slug: Option<&str>) -> String {
    format!("{domain_slug}/{}", sub_slug.unwrap_or("-"))
}

/// The previous features of an unchanged unit, unless there are none or one
/// of their slugs was taken meanwhile by a newly derived feature (slugs are
/// unique across domains, use cases refer to their feature by slug alone):
/// that forces a fresh derivation.
fn reusable(previous: &[Feature], features: &[Feature], unit: &Unit<'_>) -> Option<Vec<Feature>> {
    let kept: Vec<&Feature> = previous
        .iter()
        .filter(|f| {
            f.domain_slug == unit.domain.slug && f.sub_domain_slug.as_deref() == unit.sub_slug
        })
        .collect();
    let collides = kept
        .iter()
        .any(|k| features.iter().any(|f| f.slug == k.slug));
    (!kept.is_empty() && !collides).then(|| kept.into_iter().cloned().collect())
}

/// The LLM's features turned into [`Feature`]s: those citing no file of the
/// unit are dropped, slugs made unique against `existing` and each other.
fn validated_features(raw: RawFeatures, unit: &Unit<'_>, existing: &[Feature]) -> Vec<Feature> {
    let known: BTreeSet<&PathBuf> = unit.paths.iter().collect();
    let mut produced: Vec<Feature> = Vec::new();
    for raw_feature in raw.features {
        let files = known_files(&raw_feature.files, &known);
        if files.is_empty() {
            tracing::warn!(
                feature = %raw_feature.slug,
                "feature dropped: it cites no file from its domain"
            );
            continue;
        }
        let taken = existing.iter().chain(&produced).map(|f| f.slug.as_str());
        let slug = unique_slug(&raw_feature.slug, taken);
        produced.push(Feature {
            slug,
            domain_slug: unit.domain.slug.clone(),
            sub_domain_slug: unit.sub_slug.map(str::to_string),
            name: raw_feature.name,
            description: raw_feature.description,
            source_paths: files,
            confidence: None,
        });
    }
    produced
}

/// The cited files that belong to the unit, as strings.
fn known_files(cited: &[PathBuf], known: &BTreeSet<&PathBuf>) -> Vec<String> {
    cited
        .iter()
        .filter(|f| known.contains(f))
        .map(|f| f.display().to_string())
        .collect()
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
        features.push(old.clone());
    }
    warn_on_error(prints.save(repo_root));
    warn_on_error(save_features(repo_root, &features));
}

fn features_prompt(
    brief: &ProductBrief,
    domain_name: &str,
    domain_description: &str,
    sub_domain_name: &str,
    evidence: &str,
    files: &[&FileSummary],
) -> String {
    let mut prompt = format!(
        "{}Domain: {domain_name} — {domain_description}\n",
        brief.prompt_head()
    );
    if !sub_domain_name.is_empty() {
        let _ = writeln!(prompt, "Sub-domain: {sub_domain_name}");
    }
    if !evidence.is_empty() {
        let _ = write!(prompt, "\n{evidence}");
    }

    let mut ranked: Vec<&&FileSummary> = files.iter().collect();
    ranked.sort_by(|a, b| {
        b.commit_count
            .cmp(&a.commit_count)
            .then(a.path.cmp(&b.path))
    });
    let omitted = ranked.len().saturating_sub(MAX_FILES_PER_PROMPT);
    ranked.truncate(MAX_FILES_PER_PROMPT);

    let _ = writeln!(prompt, "\nFiles:");
    for file in ranked {
        let _ = writeln!(prompt, "- {}: {}", file.path.display(), file.role_summary);
    }
    if omitted > 0 {
        let _ = writeln!(
            prompt,
            "({omitted} less-modified file(s) omitted from this list)"
        );
    }
    prompt
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
