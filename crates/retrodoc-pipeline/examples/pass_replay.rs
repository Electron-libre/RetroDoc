//! Replays one pass alone on frozen inputs, N times, and scores each run against the hand-made reference
//! (`issues/benchmark_passes_on_fixed_inputs.md`).
//!
//! Usage:
//! `DEEPSEEK_API_KEY=x cargo run -q -p retrodoc-pipeline --example pass_replay -- <pass> <frozen dir> <benchmark dir> <model> [runs] [--freeze-first]`
//!
//! - `pass`: `domains` (the clustering of the surface), `features` (on the frozen domains) or `use-cases` (on
//!   the frozen features; it reads the code, of the tree at `manifest.source_run`).
//! - `frozen dir`: `~/retrodoc-runs/frozen/<repo>` (see the `freeze_inputs` example).
//! - `benchmark dir`: `benchmark/<repo>` (`reference.yaml`, `matches.yaml`).
//! - `model`: a `DeepSeek` model, e.g. `deepseek-chat`. `runs`: default 3.
//! - `--freeze-first` (domains or features): the result of the first run replaces the frozen one, which the
//!   next replay needs (the domains and features of the `files` path hold paths, not what the `surface` path
//!   reads).
//!
//! Each run starts from an empty work directory and the temperature is the pipeline's for the pass. The
//! same model then proposes pairs for the names no pair of `matches.yaml` covers (the "judge" columns: an
//! LLM's reading, not the strict one). Prints one Markdown table.

use std::path::Path;

use retrodoc_core::config::{LlmConfig, ZERO_TEMPERATURE_PASSES};
use retrodoc_llm::{OpenRouterProvider, UsageProvider, UsageTracker};
use retrodoc_pipeline::benchmark::{
    replay_domains, replay_features, replay_table, replay_use_cases, use_case_table, FrozenInputs,
    Matches, Reference, RunFigures, Scoring, UseCaseFigures,
};
use serde_json::json;

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let freeze_first = args.iter().any(|a| a == "--freeze-first");
    args.retain(|a| a != "--freeze-first");
    let [pass, frozen_dir, bench_dir, model, rest @ ..] = args.as_slice() else {
        eprintln!("usage: pass_replay <domains|features|use-cases> <frozen dir> <benchmark dir> <model> [runs] [--freeze-first]");
        std::process::exit(2);
    };
    let runs: usize = rest.first().and_then(|n| n.parse().ok()).unwrap_or(3);
    let result = run(
        pass,
        Path::new(frozen_dir),
        Path::new(bench_dir),
        model,
        runs,
        freeze_first,
    )
    .await;
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run(
    pass: &str,
    frozen_dir: &Path,
    bench_dir: &Path,
    model: &str,
    runs: usize,
    freeze_first: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !["domains", "features", "use-cases"].contains(&pass)
        || (freeze_first && pass == "use-cases")
    {
        return Err(
            "pass is `domains`, `features` or `use-cases`; --freeze-first goes with the first two"
                .into(),
        );
    }
    let mut frozen = FrozenInputs::open(frozen_dir)?;
    let reference = Reference::load(&bench_dir.join("reference.yaml"))?;
    let matches_path = bench_dir.join("matches.yaml");
    let matches = Matches::load(&matches_path)?;
    let mut config: LlmConfig = serde_json::from_value(json!({
        "provider": "deepseek",
        "model": model,
        "timeout_secs": 600,
    }))?;
    if ZERO_TEMPERATURE_PASSES.contains(&pass) {
        config.temperature = Some(0.0);
    }

    if pass == "use-cases" {
        let tree = std::path::PathBuf::from(&frozen.manifest.source_run);
        let mut figures: Vec<UseCaseFigures> = Vec::new();
        for n in 1..=runs {
            let tracker = UsageTracker::new();
            let llm =
                UsageProvider::new(OpenRouterProvider::from_config(&config)?, tracker.clone());
            let work = tempfile::tempdir()?;
            let run =
                replay_use_cases(&frozen, &tree, work.path(), &llm, Some(&llm), &tracker).await?;
            eprintln!("run {n}/{runs}: {} use cases", run.use_cases);
            figures.push(run);
        }
        println!(
            "Frozen inputs written by `{}` at commit {}, tree {}.\n",
            frozen.manifest.model,
            frozen.manifest.commit,
            tree.display()
        );
        println!(
            "{}",
            use_case_table(
                &format!("use cases of {} with {model}", frozen.manifest.repo),
                &figures
            )
        );
        return Ok(());
    }

    let mut figures: Vec<RunFigures> = Vec::new();
    for n in 1..=runs {
        let tracker = UsageTracker::new();
        let llm = UsageProvider::new(OpenRouterProvider::from_config(&config)?, tracker.clone());
        let work = tempfile::tempdir()?;
        let scoring = Scoring {
            reference: &reference,
            matches: &matches,
            matches_path: &matches_path,
        };
        let run = if pass == "domains" {
            replay_domains(&frozen, work.path(), &llm, Some(&llm), &tracker, scoring).await?
        } else {
            replay_features(&frozen, work.path(), &llm, Some(&llm), &tracker, scoring).await?
        };
        if freeze_first && n == 1 {
            let origin = format!("written from the surface by {model}, first run of pass_replay");
            if pass == "domains" {
                frozen.replace_domains(work.path(), &origin)?;
            } else {
                frozen.replace_features(work.path(), &origin)?;
            }
        }
        eprintln!("run {n}/{runs}: {} {pass}", run.items);
        figures.push(run);
    }
    println!(
        "Frozen inputs written by `{}` at commit {}{}.\n",
        frozen.manifest.model,
        frozen.manifest.commit,
        [
            ("domains", &frozen.manifest.domains_origin),
            ("features", &frozen.manifest.features_origin)
        ]
        .iter()
        .filter_map(|(what, origin)| origin.as_ref().map(|o| format!(", {what} {o}")))
        .collect::<String>()
    );
    println!(
        "{}",
        replay_table(
            &format!("{pass} of {} with {model}", frozen.manifest.repo),
            &figures
        )
    );
    Ok(())
}
