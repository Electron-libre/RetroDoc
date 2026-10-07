//! Matching what a run generated with the reference, and the recall and precision that follow.
//!
//! A generated name matches a reference name when a hand-written pair in `matches.yaml` says so, or,
//! failing that, when both are the same name once normalized (`naming::normalize`). The manual pairs
//! take precedence: an entry lets the user correct the automatic reading, and the LLM judge (a later
//! step) only looks at what is still unmatched.
//!
//! - recall: share of the reference items some generated item matches;
//! - precision: share of the generated items that match some reference item. Several generated items
//!   may stand for one reference item (a split), which is fine for recall and counts for precision.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::reference::Reference;
use crate::domains::{DomainMap, UNCATEGORIZED_SLUG};
use crate::error::PipelineError;
use crate::features::load_features;
use crate::naming::normalize;

/// `benchmark/<repo>/matches.yaml`: the pairs written by hand, kept between runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Matches {
    #[serde(default)]
    pub domains: Vec<Pair>,
    #[serde(default)]
    pub features: Vec<Pair>,
}

/// A generated name and the reference name it stands for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// `(generated, reference)`, in the order of the generated items.
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
    for name in generated {
        let wanted = normalize(name);
        let paired = pairs.iter().find(|p| normalize(&p.generated) == wanted);
        let found = match paired {
            Some(pair) => reference_index(&pair.reference),
            None => reference_index(name),
        };
        match found {
            Some(index) => {
                covered[index] = true;
                matched.push((name.clone(), reference[index].clone()));
            }
            None => unmatched_generated.push(name.clone()),
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
        precision: share(matched.len(), generated.len()),
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

    #[test]
    fn a_manual_pair_wins_over_the_name() {
        // "Billing" is also a reference name, but the user says it stands for "Payments".
        let score = score_of(
            &["Billing"],
            &["Billing", "Payments"],
            &[pair("billing", "Payments")],
        )
        .unwrap();
        assert_eq!(
            score.matched,
            [("Billing".to_string(), "Payments".to_string())]
        );
        assert_eq!(score.unmatched_reference, ["Billing"]);
        assert_eq!(score.recall, Some(0.5));
        assert_eq!(score.precision, Some(1.0));
    }

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

    #[test]
    fn a_pair_for_a_generated_name_absent_from_this_run_is_ignored() {
        let score = score_of(&["A"], &["B"], &[pair("Gone", "B")]).unwrap();
        assert_eq!(score.matched, []);
        assert_eq!(score.recall, Some(0.0));
    }

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
        };
        let map = DomainMap {
            domains: vec![
                cluster("ordering", "Ordering"),
                cluster(UNCATEGORIZED_SLUG, "Uncategorized"),
            ],
        };
        save_yaml(&Artifact::Domains.path(root), &map).unwrap();
        let feature = |name: &str| Feature {
            slug: name.to_lowercase(),
            domain_slug: "ordering".to_string(),
            sub_domain_slug: None,
            name: name.to_string(),
            description: String::new(),
            source_paths: vec![],
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
