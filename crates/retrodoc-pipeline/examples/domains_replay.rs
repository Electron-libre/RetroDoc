//! Replays the domains pass alone on frozen inputs, N times, and scores each run against the hand-made
//! reference (`issues/benchmark_passes_on_fixed_inputs.md`).
//!
//! Usage:
//! `DEEPSEEK_API_KEY=x cargo run -q -p retrodoc-pipeline --example domains_replay -- <frozen dir> <benchmark dir> <model> [runs]`
//!
//! - `frozen dir`: `~/retrodoc-runs/frozen/<repo>` (see the `freeze_inputs` example).
//! - `benchmark dir`: `benchmark/<repo>` (`reference.yaml`, `matches.yaml`).
//! - `model`: a `DeepSeek` model, e.g. `deepseek-chat`. `runs`: default 3.
//!
//! The clustering of the surface (glossary entities and entry-point resources) is asked each time from an
//! empty work directory, then the same model proposes pairs for the names no pair of `matches.yaml` covers
//! (the "judge" columns: an LLM's reading, not the strict one); the temperature is the pipeline's for the pass. Prints one Markdown table.

use std::path::Path;

use retrodoc_core::config::{LlmConfig, ZERO_TEMPERATURE_PASSES};
use retrodoc_llm::{OpenRouterProvider, UsageProvider, UsageTracker};
use retrodoc_pipeline::benchmark::{
    replay_domains, replay_table, FrozenInputs, Matches, Reference, RunFigures, Scoring,
};
use serde_json::json;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [frozen_dir, bench_dir, model, rest @ ..] = args.as_slice() else {
        eprintln!("usage: domains_replay <frozen dir> <benchmark dir> <model> [runs]");
        std::process::exit(2);
    };
    let runs: usize = rest.first().and_then(|n| n.parse().ok()).unwrap_or(3);
    if let Err(error) = run(Path::new(frozen_dir), Path::new(bench_dir), model, runs).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run(
    frozen_dir: &Path,
    bench_dir: &Path,
    model: &str,
    runs: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let frozen = FrozenInputs::open(frozen_dir)?;
    let reference = Reference::load(&bench_dir.join("reference.yaml"))?;
    let matches_path = bench_dir.join("matches.yaml");
    let matches = Matches::load(&matches_path)?;
    let mut config: LlmConfig = serde_json::from_value(json!({
        "provider": "deepseek",
        "model": model,
        "timeout_secs": 600,
    }))?;
    if ZERO_TEMPERATURE_PASSES.contains(&"domains") {
        config.temperature = Some(0.0);
    }

    let mut figures: Vec<RunFigures> = Vec::new();
    for n in 1..=runs {
        let tracker = UsageTracker::new();
        let llm = UsageProvider::new(OpenRouterProvider::from_config(&config)?, tracker.clone());
        let work = tempfile::tempdir()?;
        let run = replay_domains(
            &frozen,
            work.path(),
            &llm,
            Some(&llm),
            &tracker,
            Scoring {
                reference: &reference,
                matches: &matches,
                matches_path: &matches_path,
            },
        )
        .await?;
        eprintln!("run {n}/{runs}: {} domains", run.items);
        figures.push(run);
    }
    println!(
        "Frozen inputs written by `{}` at commit {}.\n",
        frozen.manifest.model, frozen.manifest.commit
    );
    println!(
        "{}",
        replay_table(
            &format!("Domains of {} with {model}", frozen.manifest.repo),
            &figures
        )
    );
    Ok(())
}
