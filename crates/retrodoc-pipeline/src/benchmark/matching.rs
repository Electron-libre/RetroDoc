//! Matching what a run generated with the reference, and the recall and precision that follow.
//!
//! A generated name matches the reference name that is the same once normalized (`naming::normalize`),
//! and every reference name a hand-written pair in `matches.yaml` gives it: a generated item that
//! merges several capabilities may stand for several reference items. The LLM judge (a later step)
//! only proposes reference items that nothing covers yet.
//!
//! - recall: share of the reference items some generated item matches;
//! - precision: share of the generated items that match some reference item, each counted once
//!   whatever the number of reference items it stands for. Several generated items may stand for one
//!   reference item (a split), which is fine for recall and counts for precision.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::reference::Reference;
use crate::domains::{DomainMap, UNCATEGORIZED_SLUG};
use crate::error::PipelineError;
use crate::features::load_features;
use crate::naming::normalize;

/// The version of the scoring rules, saved in each [`super::RunReport`]: bump it when [`score`] or the
/// judge changes what counts, so `benchmark-table` doesn't read a change of rules as a change of quality.
/// Version 1: a generated item counts for each reference item it stands for.
pub const SCORING_VERSION: u32 = 1;

/// `benchmark/<repo>/matches.yaml`: the pairs written by hand, kept between runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Matches {
    #[serde(default)]
    pub domains: Vec<Pair>,
    #[serde(default)]
    pub features: Vec<Pair>,
}

/// A generated name and the reference name it stands for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Pair {
    pub generated: String,
    pub reference: String,
}

/// How well the generated items cover the reference ones, and what is left to read by hand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    /// `None` when the reference is empty.
    pub recall: Option<f32>,
    /// `None` when nothing was generated.
    pub precision: Option<f32>,
    /// `(generated, reference)`, in the order of the generated items, one per reference item a
    /// generated name stands for.
    pub matched: Vec<(String, String)>,
    pub unmatched_generated: Vec<String>,
    pub unmatched_reference: Vec<String>,
}

impl Matches {
    /// Reads `matches.yaml`; a missing file is no pair at all (the first run has none yet).
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but can't be read or parsed.
    pub fn load(path: &Path) -> Result<Self, PipelineError> {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(source) => {
                return Err(PipelineError::ArtifactIo {
                    path: path.to_path_buf(),
                    source,
                })
            }
        };
        serde_yaml::from_str(&raw).map_err(|error| PipelineError::InvalidReference {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })
    }
}

/// Scores `generated` names against `reference` names.
///
/// # Errors
///
/// Returns [`PipelineError::InvalidReference`] if a manual pair names a reference item that doesn't
/// exist (a typo would otherwise count as a miss forever). A pair whose generated name isn't in this
/// run is ignored: the pairs outlive the runs they were written for.
pub fn score(
    generated: &[String],
    reference: &[String],
    pairs: &[Pair],
    matches_path: &Path,
) -> Result<Score, PipelineError> {
    let reference_index = |name: &str| {
        let wanted = normalize(name);
        reference.iter().position(|r| normalize(r) == wanted)
    };
    for pair in pairs {
        if reference_index(&pair.reference).is_none() {
            return Err(PipelineError::InvalidReference {
                path: matches_path.to_path_buf(),
                reason: format!(
                    "`{}` is paired with `{}`, which is not in the reference",
                    pair.generated, pair.reference
                ),
            });
        }
    }

    let mut matched = Vec::new();
    let mut unmatched_generated = Vec::new();
    let mut covered = vec![false; reference.len()];
    let mut matched_generated = 0;
    for name in generated {
        let wanted = normalize(name);
        let mut found: Vec<usize> = reference_index(name).into_iter().collect();
        for pair in pairs.iter().filter(|p| normalize(&p.generated) == wanted) {
            if let Some(index) = reference_index(&pair.reference) {
                if !found.contains(&index) {
                    found.push(index);
                }
            }
        }
        if found.is_empty() {
            unmatched_generated.push(name.clone());
            continue;
        }
        matched_generated += 1;
        for index in found {
            covered[index] = true;
            matched.push((name.clone(), reference[index].clone()));
        }
    }
    let unmatched_reference = reference
        .iter()
        .zip(&covered)
        .filter(|(_, covered)| !**covered)
        .map(|(name, _)| name.clone())
        .collect();
    Ok(Score {
        recall: share(covered.iter().filter(|c| **c).count(), reference.len()),
        precision: share(matched_generated, generated.len()),
        matched,
        unmatched_generated,
        unmatched_reference,
    })
}

/// Domains and features of one run against the reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub domains: Score,
    pub features: Score,
}

/// Scores the domains and features saved in `repo_root` (`.retrodoc/cache/`) against `reference`.
/// `None` when the run has no clustering (nothing generated).
///
/// # Errors
///
/// Same as [`score`].
pub fn compare(
    repo_root: &Path,
    reference: &Reference,
    matches: &Matches,
    matches_path: &Path,
) -> Result<Option<Comparison>, PipelineError> {
    let Some(map) = DomainMap::load(repo_root) else {
        return Ok(None);
    };
    let domains: Vec<String> = map
        .domains
        .iter()
        .filter(|d| d.slug != UNCATEGORIZED_SLUG)
        .map(|d| d.name.clone())
        .collect();
    let features: Vec<String> = load_features(repo_root)
        .unwrap_or_default()
        .into_iter()
        .map(|f| f.name)
        .collect();
    let reference_domains: Vec<String> = reference.domains.iter().map(|d| d.name.clone()).collect();
    let reference_features: Vec<String> = reference
        .domains
        .iter()
        .flat_map(|d| d.features.iter().map(|f| f.name.clone()))
        .collect();
    Ok(Some(Comparison {
        domains: score(&domains, &reference_domains, &matches.domains, matches_path)?,
        features: score(
            &features,
            &reference_features,
            &matches.features,
            matches_path,
        )?,
    }))
}

#[allow(clippy::cast_precision_loss)]
fn share(part: usize, whole: usize) -> Option<f32> {
    (whole > 0).then(|| part as f32 / whole as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    fn pair(generated: &str, reference: &str) -> Pair {
        Pair {
            generated: generated.to_string(),
            reference: reference.to_string(),
        }
    }

    fn score_of(
        generated: &[&str],
        reference: &[&str],
        pairs: &[Pair],
    ) -> Result<Score, PipelineError> {
        score(
            &names(generated),
            &names(reference),
            pairs,
            Path::new("matches.yaml"),
        )
    }

    #[test]
    fn same_name_once_normalized_matches_without_a_pair() {
        let score = score_of(
            &["order-management", "Billing"],
            &["Order Management", "Shipping"],
            &[],
        )
        .unwrap();
        assert_eq!(
            score.matched,
            [(
                "order-management".to_string(),
                "Order Management".to_string()
            )]
        );
        assert_eq!(score.unmatched_generated, ["Billing"]);
        assert_eq!(score.unmatched_reference, ["Shipping"]);
        assert_eq!(score.recall, Some(0.5));
        assert_eq!(score.precision, Some(0.5));
    }

    // spec: BSC-1
    #[test]
    fn a_pair_adds_to_the_name() {
        // "Billing" is also a reference name, and the user says it holds "Payments" as well.
        let score = score_of(
            &["Billing"],
            &["Billing", "Payments", "Shipping"],
            &[pair("billing", "Payments")],
        )
        .unwrap();
        assert_eq!(
            score.matched,
            [
                ("Billing".to_string(), "Billing".to_string()),
                ("Billing".to_string(), "Payments".to_string())
            ]
        );
        assert_eq!(score.unmatched_reference, ["Shipping"]);
        assert!((score.recall.unwrap() - 2.0 / 3.0).abs() < 1e-6);
    }

    // spec: BSC-1, BSC-2
    #[test]
    fn a_merge_counts_for_each_reference_item_and_once_for_precision() {
        let score = score_of(
            &["Voting and flagging", "A", "B", "C"],
            &["Upvote", "Flag", "Hide", "Other"],
            &[
                pair("Voting and flagging", "Upvote"),
                pair("Voting and flagging", "Flag"),
                pair("Voting and flagging", "Hide"),
                pair("Voting and flagging", "flag"),
            ],
        )
        .unwrap();
        assert_eq!(score.recall, Some(0.75), "a repeated pair counts once");
        assert_eq!(score.precision, Some(0.25));
        assert_eq!(score.matched.len(), 3);
        assert_eq!(score.unmatched_generated, ["A", "B", "C"]);
        assert_eq!(score.unmatched_reference, ["Other"]);
    }

    // spec: BSC-2
    #[test]
    fn a_split_counts_once_for_recall_and_each_time_for_precision() {
        let score = score_of(
            &["Card payment", "Bank transfer", "Noise"],
            &["Payments"],
            &[
                pair("Card payment", "Payments"),
                pair("Bank transfer", "Payments"),
            ],
        )
        .unwrap();
        assert_eq!(score.recall, Some(1.0));
        assert!((score.precision.unwrap() - 2.0 / 3.0).abs() < 1e-6);
        assert_eq!(score.unmatched_generated, ["Noise"]);
    }

    // spec: BSC-3
    #[test]
    fn a_pair_for_a_generated_name_absent_from_this_run_is_ignored() {
        let score = score_of(&["A"], &["B"], &[pair("Gone", "B")]).unwrap();
        assert_eq!(score.matched, []);
        assert_eq!(score.recall, Some(0.0));
    }

    // spec: BSC-3
    #[test]
    fn a_pair_naming_an_unknown_reference_item_is_an_error() {
        let error = score_of(&["A"], &["B"], &[pair("A", "Typo")])
            .unwrap_err()
            .to_string();
        assert!(error.contains("`Typo`"), "{error}");
        assert!(error.contains("matches.yaml"), "{error}");
    }

    #[test]
    fn nothing_to_divide_by_is_no_score() {
        let score = score_of(&[], &["B"], &[]).unwrap();
        assert_eq!((score.recall, score.precision), (Some(0.0), None));
        let score = score_of(&["A"], &[], &[]).unwrap();
        assert_eq!((score.recall, score.precision), (None, Some(0.0)));
    }

    #[test]
    fn compares_the_saved_domains_and_features_with_the_reference() {
        use crate::artifact::{save_yaml, Artifact};
        use crate::domains::DomainCluster;
        use crate::features::save_features;
        use retrodoc_core::model::Feature;

        let reference: Reference = serde_yaml::from_str(
            "repository: r\ncommit: c\npurpose: p\nactors: []\ndomains:\n  - name: Ordering\n    description: d\n    features:\n      - {name: Checkout, description: d}\n      - {name: Refund, description: d}\n",
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(
            compare(root, &reference, &Matches::default(), Path::new("m.yaml")).unwrap(),
            None,
            "nothing generated"
        );

        let cluster = |slug: &str, name: &str| DomainCluster {
            slug: slug.to_string(),
            name: name.to_string(),
            description: String::new(),
            paths: vec![],
            sub_domains: vec![],
            ..DomainCluster::default()
        };
        let map = DomainMap {
            domains: vec![
                cluster("ordering", "Ordering"),
                cluster(UNCATEGORIZED_SLUG, "Uncategorized"),
            ],
        };
        save_yaml(&Artifact::Domains.path(root), &map).unwrap();
        let feature = |name: &str| Feature {
            back_office: false,
            slug: name.to_lowercase(),
            domain_slug: "ordering".to_string(),
            sub_domain_slug: None,
            name: name.to_string(),
            description: String::new(),
            source_paths: vec![],
            entry_points: Vec::new(),
            confidence: None,
        };
        save_features(root, &[feature("Checkout"), feature("Newsletter")]).unwrap();

        let comparison = compare(root, &reference, &Matches::default(), Path::new("m.yaml"))
            .unwrap()
            .unwrap();

        assert_eq!(comparison.domains.recall, Some(1.0));
        assert_eq!(
            comparison.domains.precision,
            Some(1.0),
            "the bucket is no domain"
        );
        assert_eq!(comparison.features.recall, Some(0.5));
        assert_eq!(comparison.features.unmatched_generated, ["Newsletter"]);
        assert_eq!(comparison.features.unmatched_reference, ["Refund"]);
    }

    #[test]
    fn loads_pairs_and_treats_a_missing_file_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("matches.yaml");
        assert_eq!(Matches::load(&path).unwrap(), Matches::default());

        std::fs::write(
            &path,
            "domains:\n  - {generated: Billing, reference: Payments}\n",
        )
        .unwrap();
        let matches = Matches::load(&path).unwrap();
        assert_eq!(matches.domains, [pair("Billing", "Payments")]);
        assert_eq!(matches.features, []);

        std::fs::write(&path, "domains: 3").unwrap();
        assert!(Matches::load(&path).is_err());
    }
}
