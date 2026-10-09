use std::path::{Path, PathBuf};

use anyhow::Context;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::benchmark::{self, Matches, Reference, RunMetrics};

use super::workspace::{repo_root, Workspace};

/// What to measure and against what.
pub struct BenchmarkOptions {
    /// The hand-written reference (`benchmark/<repo>/reference.yaml`).
    pub reference: PathBuf,
    /// The hand-written pairs; `matches.yaml` next to the reference when not given.
    pub matches: Option<PathBuf>,
    /// Also ask the LLM judge for pairs and a rating of the narratives.
    pub judge: bool,
    /// Also write the figures of the run as JSON, for `benchmark-table`.
    pub out: Option<PathBuf>,
}

/// Prints the figures of the last `generate` run in `path` and its scores against the reference.
/// Without `--judge` it makes no LLM call; with it, the judge's proposals are saved in
/// `.retrodoc/benchmark/judge.yaml` and scored apart from the hand-written pairs.
pub async fn run(
    path: &Path,
    options: &BenchmarkOptions,
    tracker: &UsageTracker,
) -> anyhow::Result<()> {
    let repo_root = repo_root(path)?;
    let reference = Reference::load(&options.reference)?;
    let matches_path = options.matches.clone().unwrap_or_else(|| {
        options
            .reference
            .parent()
            .unwrap_or(Path::new("."))
            .join("matches.yaml")
    });
    let matches = Matches::load(&matches_path)?;

    let Some(metrics) = RunMetrics::collect(&repo_root) else {
        anyhow::bail!(
            "no domains found in {} — run `retrodoc generate` first",
            repo_root.display()
        );
    };
    let Some(comparison) = benchmark::compare(&repo_root, &reference, &matches, &matches_path)?
    else {
        anyhow::bail!(
            "no domains found in {} — run `retrodoc generate` first",
            repo_root.display()
        );
    };

    let judged = if options.judge {
        let workspace = Workspace::open(&repo_root)?;
        let llm = super::usage::pass_provider(&workspace.config.llm, tracker, "benchmark judge")?;
        tracker.set_pass("benchmark judge");
        let judgement = benchmark::judge(
            llm.as_ref(),
            &repo_root,
            &reference,
            &matches,
            &matches_path,
        )
        .await
        .context("the judge failed")?
        .context("no domains found, nothing to judge")?;
        judgement.save(&repo_root)?;
        let with_judge = benchmark::compare(
            &repo_root,
            &reference,
            &matches.with_proposals(&judgement),
            &matches_path,
        )?
        .context("no domains found, nothing to judge")?;
        Some((with_judge, judgement))
    } else {
        None
    };

    print!(
        "{}",
        benchmark::summary(&metrics, &comparison, judged.as_ref().map(|(c, j)| (c, j)))
    );
    if let Some(out) = &options.out {
        let report = benchmark::RunReport::new(metrics, comparison, judged);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        std::fs::write(out, serde_json::to_string_pretty(&report)?)
            .with_context(|| format!("could not write {}", out.display()))?;
    }
    if options.judge {
        println!(
            "\nThe judge's pairs are in {}: correct them and copy the right ones to {}.",
            benchmark::Judgement::path(&repo_root).display(),
            matches_path.display()
        );
    }
    Ok(())
}

/// Prints the Markdown table of the benchmark saved in `dir` (`<dir>/<series>/*.json`, written by
/// `retrodoc benchmark --out`), with the change against `previous` when given. No LLM call.
pub fn table(dir: &Path, previous: Option<&Path>) -> anyhow::Result<()> {
    let current = benchmark::load_series(dir)?;
    if current.iter().all(|series| series.runs.is_empty()) {
        anyhow::bail!(
            "no run found in {}: expected <series>/<run>.json files",
            dir.display()
        );
    }
    let previous = previous.map(benchmark::load_series).transpose()?;
    print!("{}", benchmark::table(&current, previous.as_deref()));
    Ok(())
}
