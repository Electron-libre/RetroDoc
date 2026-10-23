//! Replays of one pass on frozen inputs (`issues/benchmark_passes_on_fixed_inputs.md`): the pass runs
//! alone on the artifacts of [`FrozenInputs`], its result is scored against the hand-made reference of the
//! level, and its calls and tokens are counted. No pass upstream varies between two replays.

use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use retrodoc_llm::{LlmProvider, UsageTracker};

use super::frozen::{FrozenInputs, ReplayPass};
use super::judge::judge;
use super::matching::{compare, Matches, Score};
use super::reference::Reference;
use crate::brief::ProductBrief;
use crate::domains::build_domains_from_surface;
use crate::entry_points::EntryPoints;
use crate::error::PipelineError;
use crate::glossary::Glossary;
use crate::surface::Surface;

/// What a replay is scored against: the reference of the repository and the pairs written by hand.
#[derive(Debug, Clone, Copy)]
pub struct Scoring<'a> {
    pub reference: &'a Reference,
    pub matches: &'a Matches,
    /// Where `matches` was read from, for the error messages.
    pub matches_path: &'a Path,
}

/// What one replay of a pass produced and cost.
#[derive(Debug, Clone, PartialEq)]
pub struct RunFigures {
    /// Items the pass produced (domains, features, ...).
    pub items: usize,
    /// The share of the reference found, and the share of what was produced that is in the reference.
    pub score: Score,
    /// The same with the pairs the judge proposed for what no pair covers (it shares the biases of an LLM:
    /// read it next to `score`, never instead); `None` when the judge did not run.
    pub judged: Option<Score>,
    pub calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub wall: Duration,
}

fn missing(frozen_dir: &Path, what: &str) -> PipelineError {
    PipelineError::InvalidReference {
        path: frozen_dir.to_path_buf(),
        reason: format!("{what} is missing or unreadable"),
    }
}

/// Runs the domains pass (clustering of the surface) once on the frozen inputs, in `work_root` (an empty
/// directory), and scores the domains against `reference`. With `judge_llm`, the judge then proposes pairs
/// for the names no pair covers; its calls are not in the figures of the pass.
///
/// # Errors
///
/// Returns an error if the frozen inputs are unusable, the pass fails or the matches can't be read.
pub async fn replay_domains(
    frozen: &FrozenInputs,
    work_root: &Path,
    llm: &dyn LlmProvider,
    judge_llm: Option<&dyn LlmProvider>,
    tracker: &UsageTracker,
    scoring: Scoring<'_>,
) -> Result<RunFigures, PipelineError> {
    frozen.prepare(ReplayPass::Domains, work_root)?;
    let glossary = Glossary::load(work_root).ok_or_else(|| missing(work_root, "glossary.yaml"))?;
    let entry_points =
        EntryPoints::load(work_root).ok_or_else(|| missing(work_root, "entry-points.yaml"))?;
    let brief = ProductBrief::load(work_root).unwrap_or_default();
    let surface = Surface::new(&glossary, &entry_points);

    let map = tracker
        .in_pass(
            "domains",
            build_domains_from_surface(work_root, &surface, &brief, llm),
        )
        .await?;
    let comparison = compare(
        work_root,
        scoring.reference,
        scoring.matches,
        scoring.matches_path,
    )?
    .ok_or_else(|| missing(work_root, "domains.yaml"))?;

    let report = tracker.report();
    let total = report.total();
    let wall = report.wall();
    let judged = match judge_llm {
        Some(judge_llm) => {
            match judge(
                judge_llm,
                work_root,
                scoring.reference,
                scoring.matches,
                scoring.matches_path,
            )
            .await?
            {
                Some(judgement) => compare(
                    work_root,
                    scoring.reference,
                    &scoring.matches.with_proposals(&judgement),
                    scoring.matches_path,
                )?
                .map(|c| c.domains),
                None => None,
            }
        }
        None => None,
    };
    Ok(RunFigures {
        items: map.domains.len(),
        score: comparison.domains,
        judged,
        calls: total.calls,
        prompt_tokens: total.prompt_tokens,
        completion_tokens: total.completion_tokens,
        wall,
    })
}

fn percent(share: Option<f32>) -> String {
    share.map_or_else(|| "-".to_string(), |s| format!("{:.0}%", s * 100.0))
}

/// The runs of one setting as a Markdown table: one row per run, then the mean and the range of each
/// figure, and the generated names no pair covers in the first run (what is left to read by hand).
#[must_use]
#[allow(clippy::cast_precision_loss)] // counts of runs and items: far below 2^52
pub fn replay_table(title: &str, runs: &[RunFigures]) -> String {
    let mut out = format!(
        "## {title}\n\n| Run | Items | Recall | Precision | Recall, judge | Precision, judge | Calls | Prompt tokens | Completion tokens | Seconds |\n|---|---|---|---|---|---|---|---|---|---|\n"
    );
    for (n, run) in runs.iter().enumerate() {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.0} |",
            n + 1,
            run.items,
            percent(run.score.recall),
            percent(run.score.precision),
            percent(run.judged.as_ref().and_then(|j| j.recall)),
            percent(run.judged.as_ref().and_then(|j| j.precision)),
            run.calls,
            run.prompt_tokens,
            run.completion_tokens,
            run.wall.as_secs_f32()
        );
    }
    let range = |values: Vec<f32>, shown: &dyn Fn(f32) -> String| -> String {
        if values.is_empty() {
            return "-".to_string();
        }
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let low = values.iter().copied().fold(f32::INFINITY, f32::min);
        let high = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        format!("{} ({}-{})", shown(mean), shown(low), shown(high))
    };
    let share = |f: fn(&RunFigures) -> Option<f32>| {
        range(runs.iter().filter_map(f).collect(), &|v| {
            format!("{:.0}%", v * 100.0)
        })
    };
    let count =
        |f: fn(&RunFigures) -> f32| range(runs.iter().map(f).collect(), &|v| format!("{v:.0}"));
    let _ = writeln!(
        out,
        "| mean (min-max) | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
        count(|r| r.items as f32),
        share(|r| r.score.recall),
        share(|r| r.score.precision),
        share(|r| r.judged.as_ref().and_then(|j| j.recall)),
        share(|r| r.judged.as_ref().and_then(|j| j.precision)),
        count(|r| r.calls as f32),
        count(|r| r.prompt_tokens as f32),
        count(|r| r.completion_tokens as f32),
        count(|r| r.wall.as_secs_f32()),
    );
    if let Some(first) = runs.first() {
        let _ = writeln!(
            out,
            "\nRun 1, generated and in no pair: {}\n\nRun 1, reference not found: {}",
            list(&first.score.unmatched_generated),
            list(&first.score.unmatched_reference)
        );
    }
    out
}

fn list(names: &[String]) -> String {
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join("; ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::Artifact;
    use crate::benchmark::frozen::Manifest;
    use crate::testing::FakeLlm;
    use retrodoc_llm::UsageProvider;

    const GLOSSARY: &str = "models:\n  customer.rb:\n    content_hash: h\n    entities:\n    - name: Customer\n      description: Someone who orders.\n";
    const ENTRY_POINTS: &str =
        "files:\n  customer.rb:\n    content_hash: h\n    entry_points: []\n";
    const REFERENCE: &str = "repository: r\ncommit: c\npurpose: p\nactors: [Customer]\ndomains:\n  - name: Customers\n    description: d\n    features:\n      - name: Order\n        description: d\n";

    fn frozen(dir: &Path) -> FrozenInputs {
        let run = dir.join("run");
        let write = |artifact: Artifact, content: &str| {
            let path = artifact.path(&run);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        };
        for artifact in [
            Artifact::Roles,
            Artifact::SignalSources,
            Artifact::Product,
            Artifact::BusinessFiles,
            Artifact::Actors,
        ] {
            write(artifact, "{}");
        }
        write(Artifact::Glossary, GLOSSARY);
        write(Artifact::EntryPoints, ENTRY_POINTS);
        let manifest = Manifest {
            repo: "demo".to_string(),
            commit: "c".to_string(),
            model: "m".to_string(),
            source_run: "run".to_string(),
        };
        FrozenInputs::freeze(&run, &dir.join("frozen"), &manifest).unwrap()
    }

    #[tokio::test]
    async fn a_domains_replay_scores_the_clustering_and_counts_the_calls() {
        let dir = tempfile::tempdir().unwrap();
        let frozen = frozen(dir.path());
        let tracker = UsageTracker::new();
        let llm = UsageProvider::new(
            FakeLlm::answering(
                r#"{"domains":[{"slug":"customers","name":"Customers","description":"d","entities":["Customer"]}]}"#,
            ),
            tracker.clone(),
        );
        let reference: Reference = serde_yaml::from_str(REFERENCE).unwrap();

        let figures = replay_domains(
            &frozen,
            &dir.path().join("work"),
            &llm,
            None,
            &tracker,
            Scoring {
                reference: &reference,
                matches: &Matches::default(),
                matches_path: Path::new("matches.yaml"),
            },
        )
        .await
        .unwrap();

        assert_eq!(figures.items, 1);
        assert_eq!(figures.score.recall, Some(1.0));
        assert_eq!(figures.score.precision, Some(1.0));
        assert_eq!(figures.calls, 1);
        let text = replay_table("demo", &[figures]);
        assert!(
            text.contains("| 1 | 1 | 100% | 100% | - | - | 1 |"),
            "{text}"
        );
        assert!(text.contains("mean (min-max)"), "{text}");
    }

    #[tokio::test]
    async fn a_replay_leaves_the_frozen_inputs_alone() {
        let dir = tempfile::tempdir().unwrap();
        let frozen = frozen(dir.path());
        let tracker = UsageTracker::new();
        let llm = UsageProvider::new(FakeLlm::answering(r#"{"domains":[]}"#), tracker.clone());
        let reference: Reference = serde_yaml::from_str(REFERENCE).unwrap();

        replay_domains(
            &frozen,
            &dir.path().join("work"),
            &llm,
            None,
            &tracker,
            Scoring {
                reference: &reference,
                matches: &Matches::default(),
                matches_path: Path::new("matches.yaml"),
            },
        )
        .await
        .unwrap();

        assert!(!dir.path().join("frozen/cache/domains.yaml").exists());
    }

    #[tokio::test]
    async fn the_judge_pairs_what_the_names_do_not() {
        let dir = tempfile::tempdir().unwrap();
        let frozen = frozen(dir.path());
        let tracker = UsageTracker::new();
        let llm = UsageProvider::new(
            FakeLlm::sequence(&[
                r#"{"domains":[{"slug":"clients","name":"Clients","description":"d","entities":["Customer"]}]}"#,
                r#"{"matches":[{"generated":"Clients","reference":"Customers"}]}"#,
            ]),
            tracker.clone(),
        );
        let reference: Reference = serde_yaml::from_str(REFERENCE).unwrap();

        let figures = replay_domains(
            &frozen,
            &dir.path().join("work"),
            &llm,
            Some(&llm),
            &tracker,
            Scoring {
                reference: &reference,
                matches: &Matches::default(),
                matches_path: Path::new("matches.yaml"),
            },
        )
        .await
        .unwrap();

        assert_eq!(figures.score.recall, Some(0.0));
        assert_eq!(figures.judged.unwrap().recall, Some(1.0));
        assert_eq!(
            figures.calls, 1,
            "the calls of the judge are not the pass's"
        );
    }
}
