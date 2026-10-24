//! Replays one pass alone on frozen inputs, N times, and scores each run against the hand-made reference
//! (`issues/benchmark_passes_on_fixed_inputs.md`).
//!
//! Usage:
//! `DEEPSEEK_API_KEY=x cargo run -q -p retrodoc-pipeline --example pass_replay -- <domains|features|use-cases|rules> <frozen dir> <benchmark dir> <model> [runs] [--freeze-first]`
//!
//! - `pass`: `domains` (the clustering of the surface), `features` (on the frozen domains), `use-cases` (on
//!   the frozen features) or `rules` (on the frozen features and use cases). The last two read the code, of
//!   the tree at `manifest.source_run`.
//! - `frozen dir`: `~/retrodoc-runs/frozen/<repo>` (see the `freeze_inputs` example).
//! - `benchmark dir`: `benchmark/<repo>` (`reference.yaml`, `matches.yaml`).
//! - `model`: a `DeepSeek` model, e.g. `deepseek-chat`. `runs`: default 3.
//! - `--freeze-first` (domains, features or use cases): the result of the first run replaces the frozen
//!   one, which the next replay needs (what the `files` path wrote holds paths, not what the `surface` path
//!   reads).
//!
//! Each run starts from an empty work directory and the temperature is the pipeline's for the pass. The
//! same model then judges what the strict reading cannot (the "judge" columns: an LLM's reading, not the
//! strict one). Prints one Markdown table.

use std::path::{Path, PathBuf};

use retrodoc_core::config::{LlmConfig, ZERO_TEMPERATURE_PASSES};
use retrodoc_llm::{OpenRouterProvider, UsageProvider, UsageTracker};
use retrodoc_pipeline::benchmark::{
    replay_domains, replay_features, replay_rules, replay_table, replay_use_cases, rule_table,
    use_case_table, FrozenInputs, Matches, Reference, RunFigures, Scoring,
};
use serde_json::json;

type Failure = Box<dyn std::error::Error>;

struct Setup {
    pass: String,
    model: String,
    runs: usize,
    freeze_first: bool,
    frozen: FrozenInputs,
    reference: Reference,
    matches: Matches,
    matches_path: PathBuf,
    config: LlmConfig,
}

impl Setup {
    /// A provider that counts its calls in a tracker of its own, one per run.
    fn provider(&self) -> Result<(UsageProvider<OpenRouterProvider>, UsageTracker), Failure> {
        let tracker = UsageTracker::new();
        let llm = UsageProvider::new(
            OpenRouterProvider::from_config(&self.config)?,
            tracker.clone(),
        );
        Ok((llm, tracker))
    }

    fn tree(&self) -> PathBuf {
        PathBuf::from(&self.frozen.manifest.source_run)
    }

    /// What the frozen inputs are made of, for the head of the table.
    fn head(&self) -> String {
        let manifest = &self.frozen.manifest;
        let replaced: String = [
            ("domains", &manifest.domains_origin),
            ("features", &manifest.features_origin),
            ("use cases", &manifest.use_cases_origin),
        ]
        .iter()
        .filter_map(|(what, origin)| origin.as_ref().map(|o| format!(", {what} {o}")))
        .collect();
        format!(
            "Frozen inputs written by `{}` at commit {}{replaced}.\n",
            manifest.model, manifest.commit
        )
    }

    fn title(&self) -> String {
        format!(
            "{} of {} with {}",
            self.pass, self.frozen.manifest.repo, self.model
        )
    }

    fn origin(&self) -> String {
        format!(
            "written from the surface by {}, first run of pass_replay",
            self.model
        )
    }
}

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let freeze_first = args.iter().any(|a| a == "--freeze-first");
    args.retain(|a| a != "--freeze-first");
    let [pass, frozen_dir, bench_dir, model, rest @ ..] = args.as_slice() else {
        eprintln!("usage: pass_replay <domains|features|use-cases|rules> <frozen dir> <benchmark dir> <model> [runs] [--freeze-first]");
        std::process::exit(2);
    };
    let runs = rest.first().and_then(|n| n.parse().ok()).unwrap_or(3);
    let result = start(pass, frozen_dir, bench_dir, model, runs, freeze_first).await;
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn start(
    pass: &str,
    frozen_dir: &str,
    bench_dir: &str,
    model: &str,
    runs: usize,
    freeze_first: bool,
) -> Result<(), Failure> {
    if !["domains", "features", "use-cases", "rules"].contains(&pass)
        || (freeze_first && pass == "rules")
    {
        return Err("pass is `domains`, `features`, `use-cases` or `rules`; --freeze-first goes with the first three".into());
    }
    let bench_dir = Path::new(bench_dir);
    let matches_path = bench_dir.join("matches.yaml");
    let mut config: LlmConfig = serde_json::from_value(json!({
        "provider": "deepseek",
        "model": model,
        "timeout_secs": 600,
    }))?;
    if ZERO_TEMPERATURE_PASSES.contains(&pass) {
        config.temperature = Some(0.0);
    }
    let mut setup = Setup {
        pass: pass.to_string(),
        model: model.to_string(),
        runs,
        freeze_first,
        frozen: FrozenInputs::open(Path::new(frozen_dir))?,
        reference: Reference::load(&bench_dir.join("reference.yaml"))?,
        matches: Matches::load(&matches_path)?,
        matches_path,
        config,
    };
    match pass {
        "use-cases" => use_cases(&mut setup).await,
        "rules" => rules(&setup).await,
        _ => surface_pass(&mut setup).await,
    }
}

async fn rules(setup: &Setup) -> Result<(), Failure> {
    let mut figures = Vec::new();
    for n in 1..=setup.runs {
        let (llm, tracker) = setup.provider()?;
        let work = tempfile::tempdir()?;
        let run = replay_rules(
            &setup.frozen,
            &setup.tree(),
            work.path(),
            &llm,
            Some(&llm),
            &tracker,
            &setup.reference,
        )
        .await?;
        eprintln!("run {n}/{}: {} rules", setup.runs, run.published);
        figures.push(run);
    }
    println!("{}", setup.head());
    println!("{}", rule_table(&setup.title(), &figures));
    Ok(())
}

async fn use_cases(setup: &mut Setup) -> Result<(), Failure> {
    let mut figures = Vec::new();
    for n in 1..=setup.runs {
        let (llm, tracker) = setup.provider()?;
        let work = tempfile::tempdir()?;
        let run = replay_use_cases(
            &setup.frozen,
            &setup.tree(),
            work.path(),
            &llm,
            Some(&llm),
            &tracker,
        )
        .await?;
        if setup.freeze_first && n == 1 {
            let origin = setup.origin();
            setup.frozen.replace_use_cases(work.path(), &origin)?;
        }
        eprintln!("run {n}/{}: {} use cases", setup.runs, run.use_cases);
        figures.push(run);
    }
    println!("{}", setup.head());
    println!("{}", use_case_table(&setup.title(), &figures));
    Ok(())
}

/// The domains and the features, scored against the reference.
async fn surface_pass(setup: &mut Setup) -> Result<(), Failure> {
    let mut figures: Vec<RunFigures> = Vec::new();
    for n in 1..=setup.runs {
        let (llm, tracker) = setup.provider()?;
        let work = tempfile::tempdir()?;
        let scoring = Scoring {
            reference: &setup.reference,
            matches: &setup.matches,
            matches_path: &setup.matches_path,
        };
        let run = if setup.pass == "domains" {
            replay_domains(
                &setup.frozen,
                work.path(),
                &llm,
                Some(&llm),
                &tracker,
                scoring,
            )
            .await?
        } else {
            replay_features(
                &setup.frozen,
                work.path(),
                &llm,
                Some(&llm),
                &tracker,
                scoring,
            )
            .await?
        };
        if setup.freeze_first && n == 1 {
            let origin = setup.origin();
            if setup.pass == "domains" {
                setup.frozen.replace_domains(work.path(), &origin)?;
            } else {
                setup.frozen.replace_features(work.path(), &origin)?;
            }
        }
        eprintln!("run {n}/{}: {} {}", setup.runs, run.items, setup.pass);
        figures.push(run);
    }
    println!("{}", setup.head());
    println!("{}", replay_table(&setup.title(), &figures));
    Ok(())
}
