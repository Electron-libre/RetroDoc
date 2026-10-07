//! Figures read from the artifacts of one run (`.retrodoc/cache/`): sizes, how business-level the use
//! cases read, and what the run cost. No LLM call, so they are cheap and exactly reproducible from the
//! same artifacts; the comparison with a reference is a later step.

use std::path::Path;

use retrodoc_core::model::{Feature, UseCase};
use serde::{Deserialize, Serialize};

use crate::domains::{DomainMap, UNCATEGORIZED_SLUG};
use crate::features::load_features;
use crate::usage_log::{load_history, RunUsage};
use crate::use_cases::load_use_cases;

/// What the last `generate` of the clone cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cost {
    pub calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub wall_ms: u64,
    /// Calls whose server reported no token count: the token figures are then a lower bound.
    pub calls_without_usage: u64,
}

impl Cost {
    /// Sums the passes of `run`.
    #[must_use]
    pub fn of(run: &RunUsage) -> Self {
        let models = run.passes.iter().flat_map(|pass| &pass.models);
        let mut cost = Cost {
            calls: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            wall_ms: run.wall_ms,
            calls_without_usage: 0,
        };
        for model in models {
            cost.calls += model.calls;
            cost.prompt_tokens += model.prompt_tokens;
            cost.completion_tokens += model.completion_tokens;
            cost.calls_without_usage += model.calls_without_usage;
        }
        cost
    }
}

/// The deterministic figures of one run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunMetrics {
    /// Domains, without the "uncategorized" bucket.
    pub domains: usize,
    pub sub_domains: usize,
    pub features: usize,
    pub use_cases: usize,
    /// Source files no domain claims.
    pub uncategorized_files: usize,
    /// Share of the use cases that have a narrative.
    pub narrative_share: Option<f32>,
    /// Mean `business_language` score over the scored use cases.
    pub business_language: Option<f32>,
    /// Mean confidence over the scored use cases.
    pub confidence: Option<f32>,
    /// `None` when `usage.json` holds no `generate` run.
    pub cost: Option<Cost>,
}

impl RunMetrics {
    /// Reads the artifacts of the run in `repo_root`. `None` when there is no clustering: nothing was
    /// generated, which is not a run with zero figures.
    #[must_use]
    pub fn collect(repo_root: &Path) -> Option<Self> {
        let map = DomainMap::load(repo_root)?;
        let features: Vec<Feature> = load_features(repo_root).unwrap_or_default();
        let use_cases: Vec<UseCase> = load_use_cases(repo_root).unwrap_or_default();
        let real = || map.domains.iter().filter(|d| d.slug != UNCATEGORIZED_SLUG);
        Some(Self {
            domains: real().count(),
            sub_domains: real().map(|d| d.sub_domains.len()).sum(),
            features: features.len(),
            use_cases: use_cases.len(),
            uncategorized_files: map
                .domains
                .iter()
                .filter(|d| d.slug == UNCATEGORIZED_SLUG)
                .map(|d| d.paths.len() + d.sub_domains.iter().map(|s| s.paths.len()).sum::<usize>())
                .sum(),
            narrative_share: share(
                use_cases.iter().filter(|u| u.narrative.is_some()).count(),
                use_cases.len(),
            ),
            business_language: mean(
                use_cases
                    .iter()
                    .filter_map(|u| u.business_language.as_ref().map(|s| s.value)),
            ),
            confidence: mean(
                use_cases
                    .iter()
                    .filter_map(|u| u.confidence.as_ref().map(|s| s.value)),
            ),
            cost: load_history(repo_root)
                .iter()
                .rev()
                .find(|run| run.command == "generate")
                .map(Cost::of),
        })
    }
}

#[allow(clippy::cast_precision_loss)]
fn share(part: usize, whole: usize) -> Option<f32> {
    (whole > 0).then(|| part as f32 / whole as f32)
}

#[allow(clippy::cast_precision_loss)]
fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0_f32, 0_u32), |(s, n), v| (s + v, n + 1));
    (n > 0).then(|| sum / n as f32)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use retrodoc_core::model::ConfidenceScore;

    use super::*;
    use crate::artifact::{save_yaml, Artifact};
    use crate::domains::{DomainCluster, SubDomainCluster};
    use crate::features::save_features;
    use crate::usage_log::{record_run, ModelRecord, PassRecord};
    use crate::use_cases::save_use_cases;

    fn cluster(slug: &str, paths: &[&str], subs: usize) -> DomainCluster {
        DomainCluster {
            slug: slug.to_string(),
            name: slug.to_string(),
            description: String::new(),
            paths: paths.iter().map(PathBuf::from).collect(),
            sub_domains: (0..subs)
                .map(|i| SubDomainCluster {
                    slug: format!("{slug}-{i}"),
                    name: String::new(),
                    description: String::new(),
                    paths: vec![],
                })
                .collect(),
        }
    }

    fn feature(slug: &str) -> Feature {
        Feature {
            slug: slug.to_string(),
            domain_slug: "billing".to_string(),
            sub_domain_slug: None,
            name: slug.to_string(),
            description: String::new(),
            source_paths: vec![],
            confidence: None,
        }
    }

    fn use_case(narrative: bool, language: Option<f32>, confidence: Option<f32>) -> UseCase {
        UseCase {
            slug: "u".to_string(),
            feature_slug: "f".to_string(),
            name: "u".to_string(),
            description: String::new(),
            steps: vec![],
            entry_points: vec![],
            primary_actor: None,
            narrative: narrative.then(|| "A clerk invoices.".to_string()),
            business_language: language.map(|v| ConfidenceScore::new(v, None)),
            diagram_mermaid: None,
            confidence: confidence.map(|v| ConfidenceScore::new(v, None)),
        }
    }

    fn run(command: &str, calls: u64, wall_ms: u64) -> RunUsage {
        RunUsage {
            command: command.to_string(),
            finished_at: "2026-10-07T10:00:00Z".to_string(),
            wall_ms,
            passes: ["features", "use cases"]
                .iter()
                .map(|name| PassRecord {
                    name: (*name).to_string(),
                    wall_ms: 1,
                    models: vec![ModelRecord {
                        model: "m".to_string(),
                        calls,
                        calls_without_usage: 1,
                        prompt_tokens: 100 * calls,
                        completion_tokens: 10 * calls,
                    }],
                })
                .collect(),
        }
    }

    #[test]
    fn nothing_generated_is_not_a_run() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(RunMetrics::collect(dir.path()), None);
    }

    #[test]
    fn reads_sizes_scores_and_cost_of_a_run() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let map = DomainMap {
            domains: vec![
                cluster("billing", &["a.rb"], 2),
                cluster("shipping", &[], 0),
                cluster(UNCATEGORIZED_SLUG, &["x.rb", "y.rb"], 0),
            ],
        };
        save_yaml(&Artifact::Domains.path(root), &map).unwrap();
        save_features(root, &[feature("f1"), feature("f2"), feature("f3")]).unwrap();
        save_use_cases(
            root,
            &[
                use_case(true, Some(0.8), Some(0.9)),
                use_case(true, Some(0.4), None),
                use_case(false, None, Some(0.5)),
                use_case(false, None, None),
            ],
        )
        .unwrap();
        record_run(root, run("generate", 1, 5_000)).unwrap();
        record_run(root, run("roles", 7, 1)).unwrap();

        let metrics = RunMetrics::collect(root).unwrap();

        assert_eq!(
            metrics.domains, 2,
            "the uncategorized bucket is not a domain"
        );
        assert_eq!(metrics.sub_domains, 2);
        assert_eq!(metrics.features, 3);
        assert_eq!(metrics.use_cases, 4);
        assert_eq!(metrics.uncategorized_files, 2);
        assert_eq!(metrics.narrative_share, Some(0.5));
        assert!((metrics.business_language.unwrap() - 0.6).abs() < 1e-6);
        assert!((metrics.confidence.unwrap() - 0.7).abs() < 1e-6);
        assert_eq!(
            metrics.cost,
            Some(Cost {
                calls: 2,
                prompt_tokens: 200,
                completion_tokens: 20,
                wall_ms: 5_000,
                calls_without_usage: 2,
            }),
            "the last generate run counts, not the roles run recorded after it"
        );
    }

    #[test]
    fn missing_features_and_use_cases_give_zeros_and_no_scores() {
        let dir = tempfile::tempdir().unwrap();
        let map = DomainMap {
            domains: vec![cluster("billing", &[], 0)],
        };
        save_yaml(&Artifact::Domains.path(dir.path()), &map).unwrap();

        let metrics = RunMetrics::collect(dir.path()).unwrap();

        assert_eq!((metrics.features, metrics.use_cases), (0, 0));
        assert_eq!(metrics.narrative_share, None);
        assert_eq!(metrics.business_language, None);
        assert_eq!(metrics.confidence, None);
        assert_eq!(metrics.cost, None);
    }
}
