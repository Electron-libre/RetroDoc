//! Replay of the rules pass on frozen features and use cases (`issues/done/benchmark_passes_on_fixed_inputs.md`).
//!
//! The pass reads the code of its features, so the tree of the repository is copied into the work
//! directory, as for the use cases. The figures are how many rules the pass states (before and after the
//! same rule of several features is kept once), how many of the rules of the reference it states, by the
//! judge, and what the pass cost. The reference lists the rules a reader would ask about, not all of them:
//! the share of the generated rules it covers is not a precision.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use retrodoc_core::model::BusinessRule;
use retrodoc_llm::{LlmProvider, UsageTracker};
use serde::Deserialize;

use super::frame::Frame;
use super::frozen::{FrozenInputs, ReplayPass};
use super::reference::Reference;
use super::replay_use_cases::{missing, prepare_with_tree, TreeRun};
use crate::artifact::{load_yaml, Artifact};
use crate::bm25::Bm25;
use crate::error::PipelineError;
use crate::repo_map::truncate_chars;
use crate::response::complete_json;
use crate::rules::build_rules;
use crate::use_cases::load_use_cases;

/// Generated rules shown to the judge for each rule of the reference: the closest by BM25.
const CANDIDATES: usize = 12;
/// Rules of the reference asked in one call.
const BATCH: usize = 8;

/// What one replay of the rules pass produced and cost.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleFigures {
    /// Rules as the features stated them, and after the same rule of several features is kept once.
    pub stated: usize,
    pub published: usize,
    /// Rules of the reference, and how many of them a generated rule states by the judge; `None` when the
    /// judge did not run.
    pub reference_rules: usize,
    pub covered: Option<usize>,
    /// The rules of the reference no generated rule states (empty when the judge did not run).
    pub not_covered: Vec<String>,
    pub calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub wall: Duration,
}

/// Runs the rules pass once on the frozen features and use cases, in `work_root` (an empty directory),
/// reading the code of `tree`. With `judge_llm`, the judge says which rules of the reference the
/// generated rules state; its calls are not in the figures of the pass.
///
/// # Errors
///
/// Returns an error if the frozen inputs (features and use cases included) or the tree are unusable, or
/// the pass fails.
pub async fn replay_rules(
    frozen: &FrozenInputs,
    tree: &Path,
    work_root: &Path,
    llm: &dyn LlmProvider,
    judge_llm: Option<&dyn LlmProvider>,
    tracker: &UsageTracker,
    reference: &Reference,
) -> Result<RuleFigures, PipelineError> {
    let TreeRun {
        context, features, ..
    } = prepare_with_tree(
        frozen,
        ReplayPass::Rules,
        tree,
        work_root,
        &Frame::default(),
    )?;
    let use_cases =
        load_use_cases(work_root).ok_or_else(|| missing(work_root, "use-cases.yaml"))?;
    let rules = tracker
        .in_pass(
            "rules",
            build_rules(work_root, &features, &use_cases, &context, llm),
        )
        .await?;
    let report = tracker.report();
    let total = report.total();
    let wall = report.wall();
    let stated = load_yaml::<Vec<BusinessRule>>(&Artifact::RulesStated.path(work_root))
        .map_or(rules.len(), |r| r.len());

    let wanted: Vec<String> = reference.rules.clone();
    let (covered, not_covered) = match judge_llm {
        Some(judge_llm) => {
            let found = covered_by(judge_llm, &wanted, &rules).await?;
            let missed = wanted
                .iter()
                .zip(&found)
                .filter(|(_, hit)| !**hit)
                .map(|(rule, _)| rule.clone())
                .collect();
            (Some(found.iter().filter(|hit| **hit).count()), missed)
        }
        None => (None, Vec::new()),
    };
    Ok(RuleFigures {
        stated,
        published: rules.len(),
        reference_rules: wanted.len(),
        covered,
        not_covered,
        calls: total.calls,
        prompt_tokens: total.prompt_tokens,
        completion_tokens: total.completion_tokens,
        wall,
    })
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawCoverage {
    #[serde(default)]
    answers: Vec<RawAnswer>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawAnswer {
    id: usize,
    /// The number of a listed rule that states it, `0` when none does.
    #[serde(default)]
    rule: usize,
}

const COVERAGE_SYSTEM_PROMPT: &str = "You compare business rules of a software application. For each \
numbered wanted rule you get a short list of numbered candidate rules. Say which candidate states the \
same rule as the wanted one: same condition and same consequence, in other words. A candidate that is \
only about the same subject, or states a part of it, does not. Answer 0 when no candidate does. Reply \
with ONLY a single JSON object, no prose and no Markdown code fence, matching this shape: \
{\"answers\":[{\"id\":1,\"rule\":3}]}.";

/// For each wanted rule, whether one of the `rules` states it, as the judge reads them (the closest
/// candidates by BM25 are shown).
async fn covered_by(
    llm: &dyn LlmProvider,
    wanted: &[String],
    rules: &[BusinessRule],
) -> Result<Vec<bool>, PipelineError> {
    let texts: Vec<String> = rules.iter().map(|r| r.statement.clone()).collect();
    let index = Bm25::new(&texts);
    let mut found = vec![false; wanted.len()];
    for (batch_number, batch) in wanted.chunks(BATCH).enumerate() {
        let mut user = String::new();
        for (i, rule) in batch.iter().enumerate() {
            let _ = writeln!(user, "Wanted rule {}: {rule}", i + 1);
            let mut seen = BTreeSet::new();
            for (n, (doc, _)) in index
                .search_any(rule)
                .into_iter()
                .filter(|(doc, _)| seen.insert(texts[*doc].clone()))
                .take(CANDIDATES)
                .enumerate()
            {
                let _ = writeln!(
                    user,
                    "  Candidate {}: {}",
                    n + 1,
                    truncate_chars(&texts[doc], 300)
                );
            }
        }
        let Some(answer) = complete_json::<RawCoverage>(
            llm,
            COVERAGE_SYSTEM_PROMPT,
            &user,
            "benchmark judge (rules)",
        )
        .await?
        else {
            continue;
        };
        for verdict in answer.answers {
            if verdict.id >= 1 && verdict.id <= batch.len() && verdict.rule > 0 {
                found[batch_number * BATCH + verdict.id - 1] = true;
            }
        }
    }
    Ok(found)
}

/// The runs as a Markdown table, then the rules of the reference the first run does not state.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn rule_table(title: &str, runs: &[RuleFigures]) -> String {
    let mut out = format!(
        "## {title}\n\n| Run | Rules stated | Rules kept | Reference rules stated | Calls | Prompt tokens | Completion tokens | Seconds |\n|---|---|---|---|---|---|---|---|\n"
    );
    let covered = |r: &RuleFigures| {
        r.covered.map_or_else(
            || "-".to_string(),
            |c| format!("{c} of {}", r.reference_rules),
        )
    };
    for (n, run) in runs.iter().enumerate() {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {:.0} |",
            n + 1,
            run.stated,
            run.published,
            covered(run),
            run.calls,
            run.prompt_tokens,
            run.completion_tokens,
            run.wall.as_secs_f32()
        );
    }
    let range = |values: Vec<f32>| -> String {
        if values.is_empty() {
            return "-".to_string();
        }
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let low = values.iter().copied().fold(f32::INFINITY, f32::min);
        let high = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        format!("{mean:.0} ({low:.0}-{high:.0})")
    };
    let count = |f: fn(&RuleFigures) -> f32| range(runs.iter().map(f).collect());
    let _ = writeln!(
        out,
        "| mean (min-max) | {} | {} | {} | {} | {} | {} | {} |",
        count(|r| r.stated as f32),
        count(|r| r.published as f32),
        range(
            runs.iter()
                .filter_map(|r| r.covered.map(|c| c as f32))
                .collect()
        ),
        count(|r| r.calls as f32),
        count(|r| r.prompt_tokens as f32),
        count(|r| r.completion_tokens as f32),
        count(|r| r.wall.as_secs_f32()),
    );
    if let Some(first) = runs.first() {
        let _ = writeln!(
            out,
            "\nRun 1, reference rules no generated rule states: {}",
            if first.not_covered.is_empty() {
                "none".to_string()
            } else {
                first.not_covered.join(" | ")
            }
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::frozen::Manifest;
    use crate::testing::FakeLlm;
    use retrodoc_llm::UsageProvider;

    const GLOSSARY: &str = "models:\n  customer.rb:\n    content_hash: h\n    entities:\n    - name: Customer\n      description: Someone who orders.\n";
    const ENTRY_POINTS: &str =
        "files:\n  customer.rb:\n    content_hash: h\n    entry_points: []\n";
    const FEATURES: &str = "- slug: order\n  domain_slug: customers\n  name: Order\n  description: d\n  source_paths: [customer.rb]\n  entry_points: []\n";
    const USE_CASES: &str = "- slug: place-an-order\n  feature_slug: order\n  name: Place an order\n  description: d\n  steps: []\n  entry_points: []\n";
    const REFERENCE: &str = "repository: r\ncommit: c\npurpose: p\nactors: [Customer]\ndomains:\n  - name: Customers\n    description: d\n    features:\n      - name: Order\n        description: d\nrules:\n  - An order is refused when the delivery takes 60 minutes or more.\n  - A customer can have only one order.\n";

    #[tokio::test]
    async fn a_rules_replay_counts_the_rules_and_the_reference_rules_they_state() {
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("tree");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tree.join("customer.rb"), "class Customer; end\n").unwrap();
        let run = dir.path().join("run");
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
        ] {
            write(artifact, "{}");
        }
        write(Artifact::Actors, "actors: []");
        write(Artifact::Glossary, GLOSSARY);
        write(Artifact::EntryPoints, ENTRY_POINTS);
        write(Artifact::Domains, "domains: []");
        write(Artifact::Features, FEATURES);
        write(Artifact::UseCases, USE_CASES);
        let manifest = Manifest {
            repo: "demo".to_string(),
            commit: "c".to_string(),
            model: "m".to_string(),
            source_run: "run".to_string(),
            domains_origin: None,
            features_origin: None,
            use_cases_origin: None,
        };
        let frozen = FrozenInputs::freeze(&run, &dir.path().join("frozen"), &manifest).unwrap();
        let tracker = UsageTracker::new();
        let pass_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let asked = pass_calls.clone();
        let llm = UsageProvider::new(
            FakeLlm::replying(move |_, request| {
                let prompt = &request.messages[1].content;
                let judging = prompt.contains("Wanted rule");
                if !judging {
                    asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                Ok(if judging {
                    r#"{"answers":[{"id":1,"rule":1},{"id":2,"rule":0}]}"#.to_string()
                } else {
                    r#"{"rules":[{"statement":"An order is refused when its delivery takes 60 minutes or more.","kind":"limit","entities":["Customer"],"use_cases":[],"code":[{"path":"customer.rb","start_line":1,"end_line":1}],"tests":[],"docs":[]}]}"#.to_string()
                })
            }),
            tracker.clone(),
        );
        let reference: Reference = serde_yaml::from_str(REFERENCE).unwrap();

        let figures = replay_rules(
            &frozen,
            &tree,
            &dir.path().join("work"),
            &llm,
            Some(&llm),
            &tracker,
            &reference,
        )
        .await
        .unwrap();

        assert_eq!(figures.published, 1, "{figures:?}");
        assert_eq!(figures.reference_rules, 2);
        assert_eq!(figures.covered, Some(1));
        assert_eq!(figures.not_covered, ["A customer can have only one order."]);
        let pass_calls = pass_calls.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            usize::try_from(figures.calls).unwrap(),
            pass_calls,
            "the judge's calls are not the pass's"
        );
        let text = rule_table("demo", &[figures]);
        assert!(text.contains("1 of 2"), "{text}");
    }
}
