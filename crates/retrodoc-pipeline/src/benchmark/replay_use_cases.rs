//! Replay of the use cases pass on frozen features (`issues/done/benchmark_passes_on_fixed_inputs.md`).
//!
//! The pass reads the code its features are grounded on, so the replay needs the tree of the repository
//! at the commit of the frozen inputs: its files are copied into the work directory next to the frozen
//! artifacts. The figures are the ones the use cases have in a whole run (how many, with a narrative,
//! business language by the deterministic score and by the judge) plus what the pass cost. No reference
//! of use cases is matched: the reference lists a few per feature, not all of them.

use std::path::Path;
use std::time::Duration;

use retrodoc_ingest::walker::walk_repo;
use retrodoc_ingest::FileKind;
use retrodoc_llm::{LlmProvider, UsageTracker};

use super::frame::Frame;
use super::frozen::{FrozenInputs, ReplayPass};
use super::judge::{rate_narratives, NarrativeRating};
use crate::actors::Actors;

use crate::confidence::mechanical_score;
use crate::entry_points::EntryPoints;
use crate::error::PipelineError;
use crate::features::load_features;
use crate::glossary::Glossary;
use crate::slices::CodeIndex;
use crate::surface::Surface;
use crate::use_cases::{build_use_cases, UseCaseContext};
use crate::vocabulary::score_business_language;
use retrodoc_core::model::{Feature, Step};

/// Names of the main entities given to the prompts, as in `generate`.
const VOCABULARY_SIZE: usize = 40;

/// What one replay of the use cases pass produced and cost.
#[derive(Debug, Clone, PartialEq)]
pub struct UseCaseFigures {
    pub use_cases: usize,
    pub features: usize,
    /// Use cases that have a narrative.
    pub with_narrative: usize,
    /// The judge's reading of the narratives; `None` when the judge did not run.
    pub judged: Option<NarrativeRating>,
    /// Mean of the deterministic business-language score, and the share of use cases at exactly 1. It
    /// only sees identifiers leaking into the text: read it as such.
    pub mean_score: Option<f32>,
    pub share_full: Option<f32>,
    /// Steps of all the use cases, those that cite code, and those the mechanical confidence checks pass
    /// (the identifiers they name are at the cited lines, `mechanical_score`): the grounding, with no call.
    pub steps: usize,
    pub steps_cited: usize,
    pub steps_passing: usize,
    pub calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub wall: Duration,
}

pub(super) fn missing(dir: &Path, what: &str) -> PipelineError {
    PipelineError::InvalidReference {
        path: dir.to_path_buf(),
        reason: format!("{what} is missing or unreadable"),
    }
}

fn io_error(path: &Path, source: std::io::Error) -> PipelineError {
    PipelineError::ArtifactIo {
        path: path.to_path_buf(),
        source,
    }
}

/// Copies the files of the tree at `tree` that a walk keeps into `work_root`, and returns the paths of
/// those classified as source.
fn copy_tree(tree: &Path, work_root: &Path) -> Result<Vec<std::path::PathBuf>, PipelineError> {
    let files = walk_repo(tree, &[]).map_err(|error| PipelineError::InvalidReference {
        path: tree.to_path_buf(),
        reason: error.to_string(),
    })?;
    let mut sources = Vec::new();
    for file in files {
        if file.path.starts_with(".retrodoc") {
            continue;
        }
        let to = work_root.join(&file.path);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
        }
        let from = tree.join(&file.path);
        std::fs::copy(&from, &to).map_err(|e| io_error(&from, e))?;
        if file.kind == FileKind::Source {
            sources.push(file.path);
        }
    }
    Ok(sources)
}

/// What a replay of a pass that reads the code starts from.
pub(super) struct TreeRun {
    pub context: UseCaseContext,
    pub surface: Surface,
    pub features: Vec<Feature>,
}

/// Fills `work_root` with the frozen artifacts a replay of `pass` starts from and a copy of the tree, and
/// loads what the passes that read the code are given, as `generate` gives it, framed by `frame`.
pub(super) fn prepare_with_tree(
    frozen: &FrozenInputs,
    pass: ReplayPass,
    tree: &Path,
    work_root: &Path,
    frame: &Frame,
) -> Result<TreeRun, PipelineError> {
    frozen.prepare(pass, work_root)?;
    let sources = copy_tree(tree, work_root)?;
    let glossary = Glossary::load(work_root).ok_or_else(|| missing(work_root, "glossary.yaml"))?;
    let entry_points =
        EntryPoints::load(work_root).ok_or_else(|| missing(work_root, "entry-points.yaml"))?;
    let actors = Actors::load(work_root).ok_or_else(|| missing(work_root, "actors.yaml"))?;
    let features = load_features(work_root).ok_or_else(|| missing(work_root, "features.yaml"))?;
    let surface = Surface::new(&glossary, &entry_points);
    let context = UseCaseContext {
        entry_points,
        index: CodeIndex::new(sources.iter().map(std::path::PathBuf::as_path)),
        actors,
        vocabulary: surface.vocabulary(VOCABULARY_SIZE),
        brief: frame.brief(work_root),
        evidence: frame.evidence.clone(),
        ..UseCaseContext::default()
    };
    Ok(TreeRun {
        context,
        surface,
        features,
    })
}

/// Runs the use cases pass once on the frozen features, in `work_root` (an empty directory), reading the
/// code of `tree`, framed by `frame`, and measures the use cases. With `judge_llm`, the judge rates the narratives; its calls
/// are not in the figures of the pass.
///
/// # Errors
///
/// Returns an error if the frozen inputs (features included) or the tree are unusable, or the pass fails.
pub async fn replay_use_cases(
    frozen: &FrozenInputs,
    tree: &Path,
    work_root: &Path,
    llm: &dyn LlmProvider,
    judge_llm: Option<&dyn LlmProvider>,
    tracker: &UsageTracker,
    frame: &Frame,
) -> Result<UseCaseFigures, PipelineError> {
    let TreeRun {
        context,
        surface,
        features,
    } = prepare_with_tree(frozen, ReplayPass::UseCases, tree, work_root, frame)?;

    let mut use_cases = tracker
        .in_pass(
            "use-cases",
            build_use_cases(work_root, &features, &context, llm),
        )
        .await?;
    score_business_language(
        &mut use_cases,
        &surface.vocabulary(usize::MAX),
        &context.actors,
    );
    let report = tracker.report();
    let total = report.total();
    let wall = report.wall();

    let narratives: Vec<String> = use_cases
        .iter()
        .filter_map(|u| u.narrative.clone())
        .collect();
    let judged = match judge_llm {
        Some(judge_llm) => Some(rate_narratives(judge_llm, &narratives).await?),
        None => None,
    };
    let scores: Vec<f32> = use_cases
        .iter()
        .filter_map(|u| u.business_language.as_ref().map(|s| s.value))
        .collect();
    let entities: Vec<String> = Glossary::load(work_root)
        .map(|g| g.entities().map(|(_, e)| e.name.clone()).collect())
        .unwrap_or_default();
    let steps: Vec<&Step> = use_cases.iter().flat_map(|u| &u.steps).collect();
    let steps_passing = use_cases
        .iter()
        .map(|u| mechanical_score(work_root, u, &entities).0.passing)
        .sum();
    Ok(UseCaseFigures {
        use_cases: use_cases.len(),
        features: features.len(),
        with_narrative: narratives.len(),
        judged,
        mean_score: mean(&scores),
        share_full: share(scores.iter().filter(|v| **v >= 0.999).count(), scores.len()),
        steps: steps.len(),
        steps_cited: steps.iter().filter(|s| !s.source_refs.is_empty()).count(),
        steps_passing,
        calls: total.calls,
        prompt_tokens: total.prompt_tokens,
        completion_tokens: total.completion_tokens,
        wall,
    })
}

#[allow(clippy::cast_precision_loss)] // counts of use cases: far below 2^52
fn mean(values: &[f32]) -> Option<f32> {
    (!values.is_empty()).then(|| values.iter().sum::<f32>() / values.len() as f32)
}

#[allow(clippy::cast_precision_loss)]
fn share(part: usize, whole: usize) -> Option<f32> {
    (whole > 0).then(|| part as f32 / whole as f32)
}

fn percent(share: Option<f32>) -> String {
    share.map_or_else(|| "-".to_string(), |s| format!("{:.0}%", s * 100.0))
}

/// The runs as a Markdown table: one row per run, then the mean and the range of each figure.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn use_case_table(title: &str, runs: &[UseCaseFigures]) -> String {
    use std::fmt::Write as _;

    let mut out = format!(
        "## {title}\n\n| Run | Use cases | With narrative | Business language, judge | Score (mean) | Score = 1 | Steps | Cited | Checks pass | Calls | Prompt tokens | Completion tokens | Seconds |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|\n"
    );
    let judge_share = |r: &UseCaseFigures| r.judged.and_then(|j| j.business_share());
    let cited_share = |r: &UseCaseFigures| share(r.steps_cited, r.steps);
    let passing_share = |r: &UseCaseFigures| share(r.steps_passing, r.steps);
    for (n, run) in runs.iter().enumerate() {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.0} |",
            n + 1,
            run.use_cases,
            run.with_narrative,
            percent(judge_share(run)),
            percent(run.mean_score),
            percent(run.share_full),
            run.steps,
            percent(cited_share(run)),
            percent(passing_share(run)),
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
    let count =
        |f: fn(&UseCaseFigures) -> f32| range(runs.iter().map(f).collect(), &|v| format!("{v:.0}"));
    let pct = |values: Vec<f32>| range(values, &|v| format!("{:.0}%", v * 100.0));
    let _ = writeln!(
        out,
        "| mean (min-max) | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
        count(|r| r.use_cases as f32),
        count(|r| r.with_narrative as f32),
        pct(runs.iter().filter_map(judge_share).collect()),
        pct(runs.iter().filter_map(|r| r.mean_score).collect()),
        pct(runs.iter().filter_map(|r| r.share_full).collect()),
        count(|r| r.steps as f32),
        pct(runs.iter().filter_map(cited_share).collect()),
        pct(runs.iter().filter_map(passing_share).collect()),
        count(|r| r.calls as f32),
        count(|r| r.prompt_tokens as f32),
        count(|r| r.completion_tokens as f32),
        count(|r| r.wall.as_secs_f32()),
    );
    out
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
    const FEATURES: &str = "- slug: order\n  domain_slug: customers\n  name: Order\n  description: d\n  source_paths: [customer.rb]\n  entry_points: []\n";
    const USE_CASES: &str = r#"{"use_cases":[{"slug":"place-an-order","name":"Place an order","description":"A customer orders.","narrative":"A customer places an order.","steps":[{"description":"The customer is a `Customer`.","actor":{"name":"Customer","kind":"human"},"action":"orders","source_refs":[{"path":"customer.rb","start_line":1,"end_line":1}]},{"description":"Nothing to check.","actor":{"name":"System","kind":"system"},"action":"waits","source_refs":[]}]}]}"#;

    #[tokio::test]
    async fn a_use_cases_replay_reads_the_tree_and_measures_the_use_cases() {
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
        let llm = UsageProvider::new(
            FakeLlm::sequence(&[USE_CASES, r#"{"ratings":[{"id":1,"business":true}]}"#]),
            tracker.clone(),
        );

        let figures = replay_use_cases(
            &frozen,
            &tree,
            &dir.path().join("work"),
            &llm,
            Some(&llm),
            &tracker,
            &Frame::default(),
        )
        .await
        .unwrap();

        assert_eq!(figures.features, 1);
        assert_eq!(figures.use_cases, 1);
        assert_eq!(figures.with_narrative, 1);
        assert_eq!(figures.judged.unwrap().business, 1);
        assert!(figures.mean_score.is_some());
        assert_eq!(
            (figures.steps, figures.steps_cited, figures.steps_passing),
            (2, 1, 1),
            "the step that cites nothing is not grounded"
        );
        assert_eq!(figures.calls, 1, "the judge's call is not the pass's");
        let text = use_case_table("demo", &[figures]);
        assert!(text.contains("mean (min-max)"), "{text}");
        assert!(text.contains("| Steps | Cited | Checks pass |"), "{text}");
        assert!(text.contains("| 2 | 50% | 50% |"), "{text}");
    }
}
