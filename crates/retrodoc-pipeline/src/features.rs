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
use crate::domains::{DomainCluster, DomainMap, UNCATEGORIZED_SLUG};
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, Fingerprints};
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

#[derive(Debug, Deserialize)]
struct RawFeatures {
    #[serde(default)]
    features: Vec<RawFeature>,
}

#[derive(Debug, Deserialize)]
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
        let unit_print = fingerprint(unit.paths.iter().map(|p| {
            let summary = summaries.get(p.as_path()).map_or("", |f| &f.role_summary);
            format!("{}\n{summary}", p.display())
        }));
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
            &unit.domain.name,
            &unit.domain.description,
            unit.sub_name,
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
    domain_name: &str,
    domain_description: &str,
    sub_domain_name: &str,
    files: &[&FileSummary],
) -> String {
    let mut prompt = format!("Domain: {domain_name} — {domain_description}\n");
    if !sub_domain_name.is_empty() {
        let _ = writeln!(prompt, "Sub-domain: {sub_domain_name}");
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
mod tests {
    use super::*;

    use crate::domains::{DomainCluster, SubDomainCluster};
    use crate::testing::FakeLlm;

    fn file_summary(path: &str) -> FileSummary {
        FileSummary {
            path: PathBuf::from(path),
            role_summary: format!("role of {path}"),
            commit_count: 1,
            author_count: 1,
        }
    }

    fn domain(slug: &str, paths: &[&str], subs: Vec<SubDomainCluster>) -> DomainCluster {
        DomainCluster {
            slug: slug.to_string(),
            name: slug.to_string(),
            description: "d".to_string(),
            paths: paths.iter().map(PathBuf::from).collect(),
            sub_domains: subs,
        }
    }

    #[test]
    fn slugify_produces_file_name_safe_kebab_case() {
        assert_eq!(slugify("User Login / SSO!"), "user-login-sso");
        assert_eq!(slugify("../../etc"), "etc");
        assert_eq!(slugify("???"), "unnamed");
    }

    #[test]
    fn unique_slug_suffixes_collisions() {
        let taken = ["login", "login-2"];
        assert_eq!(unique_slug("Login", taken.into_iter()), "login-3");
        assert_eq!(unique_slug("logout", taken.into_iter()), "logout");
    }

    #[tokio::test]
    async fn build_features_grounds_features_on_known_files_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let repo_map = RepoMap {
            files: vec![file_summary("a.rs"), file_summary("b.rs")],
            modules: Vec::new(),
        };
        let domains = DomainMap {
            domains: vec![
                domain("billing", &["a.rs", "b.rs"], Vec::new()),
                domain(UNCATEGORIZED_SLUG, &["c.rs"], Vec::new()),
            ],
        };
        // Second feature cites only a hallucinated file → dropped; the
        // first one's hallucinated file is filtered out.
        let provider = FakeLlm::answering(
            r#"```json
            {"features":[
              {"slug":"Invoice Creation","name":"Invoice creation","description":"d",
               "files":["a.rs","ghost.rs"]},
              {"slug":"phantom","name":"Phantom","description":"d","files":["ghost.rs"]}
            ]}
            ```"#,
        );

        let features = build_features(dir.path(), &domains, &repo_map, &provider)
            .await
            .unwrap();

        assert_eq!(features.len(), 1);
        assert_eq!(features[0].slug, "invoice-creation");
        assert_eq!(features[0].domain_slug, "billing");
        assert_eq!(features[0].sub_domain_slug, None);
        assert_eq!(features[0].source_paths, vec!["a.rs".to_string()]);

        let reloaded = load_features(dir.path()).unwrap();
        assert_eq!(reloaded.len(), 1);
    }

    #[tokio::test]
    async fn build_features_covers_sub_domains_and_dedupes_slugs() {
        let dir = tempfile::tempdir().unwrap();
        let repo_map = RepoMap {
            files: vec![file_summary("a.rs"), file_summary("b.rs")],
            modules: Vec::new(),
        };
        let sub = SubDomainCluster {
            slug: "pdf".to_string(),
            name: "PDF".to_string(),
            description: "d".to_string(),
            paths: vec![PathBuf::from("b.rs")],
        };
        let domains = DomainMap {
            domains: vec![domain("billing", &["a.rs"], vec![sub])],
        };
        // Same answer for both units → the second "export" gets a suffix.
        let provider = FakeLlm::answering(
            r#"{"features":[{"slug":"export","name":"Export","description":"d",
            "files":["a.rs","b.rs"]}]}"#,
        );

        let features = build_features(dir.path(), &domains, &repo_map, &provider)
            .await
            .unwrap();

        assert_eq!(features.len(), 2);
        assert_eq!(features[0].slug, "export");
        assert_eq!(features[0].source_paths, vec!["a.rs".to_string()]);
        assert_eq!(features[1].slug, "export-2");
        assert_eq!(features[1].sub_domain_slug.as_deref(), Some("pdf"));
        assert_eq!(features[1].source_paths, vec!["b.rs".to_string()]);
    }

    #[tokio::test]
    async fn build_features_skips_a_unit_with_an_unparseable_response() {
        let dir = tempfile::tempdir().unwrap();
        let repo_map = RepoMap {
            files: vec![file_summary("a.rs")],
            modules: Vec::new(),
        };
        let domains = DomainMap {
            domains: vec![domain("billing", &["a.rs"], Vec::new())],
        };
        let provider = FakeLlm::answering("I cannot do that");

        let features = build_features(dir.path(), &domains, &repo_map, &provider)
            .await
            .unwrap();

        assert!(features.is_empty());
    }

    #[tokio::test]
    async fn rerun_reuses_unchanged_units_and_redoes_changed_ones() {
        let dir = tempfile::tempdir().unwrap();
        let domains = DomainMap {
            domains: vec![domain("billing", &["a.rs"], Vec::new())],
        };
        let map = |summary: &str| RepoMap {
            files: vec![FileSummary {
                role_summary: summary.to_string(),
                ..file_summary("a.rs")
            }],
            modules: Vec::new(),
        };
        let provider = FakeLlm::answering(
            r#"{"features":[{"slug":"f","name":"F","description":"d","files":["a.rs"]}]}"#,
        );
        let calls = || provider.calls();

        build_features(dir.path(), &domains, &map("v1"), &provider)
            .await
            .unwrap();
        assert_eq!(calls(), 1);

        let again = build_features(dir.path(), &domains, &map("v1"), &provider)
            .await
            .unwrap();
        assert_eq!(calls(), 1, "unchanged unit must not call the LLM");
        assert_eq!(again.len(), 1);

        build_features(dir.path(), &domains, &map("v2"), &provider)
            .await
            .unwrap();
        assert_eq!(
            calls(),
            2,
            "a changed file summary must invalidate the unit"
        );
    }

    #[tokio::test]
    async fn feature_slugs_are_unique_across_domains() {
        let dir = tempfile::tempdir().unwrap();
        let repo_map = RepoMap {
            files: vec![file_summary("a.rs"), file_summary("b.rs")],
            modules: Vec::new(),
        };
        let domains = DomainMap {
            domains: vec![
                domain("billing", &["a.rs"], Vec::new()),
                domain("shipping", &["b.rs"], Vec::new()),
            ],
        };
        // Both domains get the same answer, hence the same "export" slug.
        let provider = FakeLlm::answering(
            r#"{"features":[{"slug":"export","name":"Export","description":"d",
            "files":["a.rs","b.rs"]}]}"#,
        );

        let features = build_features(dir.path(), &domains, &repo_map, &provider)
            .await
            .unwrap();

        assert_eq!(features.len(), 2);
        assert_eq!(features[0].slug, "export");
        assert_eq!(features[1].slug, "export-2");
        assert_eq!(features[1].domain_slug, "shipping");
    }

    /// The answer of the call of rank `n`: one feature on `a.rs`, then `b.rs`.
    fn feature_reply(n: usize) -> String {
        format!(
            r#"{{"features":[{{"slug":"f{n}","name":"F","description":"d","files":["{}"]}}]}}"#,
            if n == 0 { "a.rs" } else { "b.rs" }
        )
    }

    #[tokio::test]
    async fn a_failed_run_keeps_the_units_done_and_the_rerun_resumes() {
        let dir = tempfile::tempdir().unwrap();
        let repo_map = RepoMap {
            files: vec![file_summary("a.rs"), file_summary("b.rs")],
            modules: Vec::new(),
        };
        let domains = DomainMap {
            domains: vec![
                domain("one", &["a.rs"], Vec::new()),
                domain("two", &["b.rs"], Vec::new()),
            ],
        };
        let flaky = FakeLlm::failing_after(1, feature_reply);
        assert!(build_features(dir.path(), &domains, &repo_map, &flaky)
            .await
            .is_err());
        assert_eq!(load_features(dir.path()).unwrap().len(), 1);

        // The first domain is cached: the second one is the call of rank 1.
        let healthy = FakeLlm::replying(|n, _| Ok(feature_reply(n + 1)));
        let features = build_features(dir.path(), &domains, &repo_map, &healthy)
            .await
            .unwrap();
        assert_eq!(features.len(), 2);
        // Only the second domain was sent to the LLM again.
        assert_eq!(healthy.calls(), 1);
    }
}
