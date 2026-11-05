//! Asks the question set of a benchmark repository to the agent of `retrodoc ask` on a saved run,
//! with all the tools then without the code tools, and judges each answer (spec `ask-question-set`,
//! `issues/ask_question_set.md`). Paid: about 90 agent requests per variant on linkding.
//!
//! Usage:
//! `DEEPSEEK_API_KEY=x cargo run -q -p retrodoc-bench --example ask_replay -- <run dir> <benchmark dir> <model> <out dir> [--max-rounds N] [--only all|docs]`
//!
//! - `run dir`: a saved run (`.retrodoc/cache/` of a `generate`, its `.retrodoc/benchmark/judge.yaml`
//!   if the benchmark judged it), e.g. a clone of the benchmark.
//! - `benchmark dir`: `benchmark/<repo>`, with `questions.yaml` and `matches.yaml`.
//! - `model`: a `DeepSeek` model, for the agent (default temperature, as `ask`) and the judge (at 0).
//! - `out dir`: where `answers-all.yaml`, `answers-docs.yaml` and `table.md` are written.
//! - `--max-rounds`: rounds of tool calls of the agent (default that of `ask`); `--only`: one variant;
//!   `--dry-run`: prints the group of each question against the run and stops, with no LLM call.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use retrodoc_agent::{Tools, DEFAULT_MAX_ROUNDS};
use retrodoc_bench::benchmark::ask_replay::{ask_questions, ask_table, RunPairs, Variant};
use retrodoc_bench::benchmark::{Judgement, Matches, QuestionSet, Reference};
use retrodoc_core::config::{Config, LlmConfig};
use retrodoc_llm::{OpenRouterProvider, UsageProvider, UsageTracker};
use retrodoc_mcp::Docs;
use serde_json::json;

type Failure = Box<dyn std::error::Error>;

const VARIANTS: [(&str, &str, Tools); 2] = [
    ("all", "all tools", Tools::All),
    ("docs", "without the code tools", Tools::DocsOnly),
];

#[tokio::main]
async fn main() {
    tracing_subscriber_init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = start(&args).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

/// Logs each judged question on stderr; quiet when the subscriber can't be set.
fn tracing_subscriber_init() {
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_target(false)
            .finish(),
    );
}

async fn start(args: &[String]) -> Result<(), Failure> {
    let usage = "usage: ask_replay <run dir> <benchmark dir> <model> <out dir> [--max-rounds N] [--only all|docs] [--dry-run]";
    let [run, bench, model, out, rest @ ..] = args else {
        return Err(usage.into());
    };
    let (mut max_rounds, mut only, mut dry_run) = (DEFAULT_MAX_ROUNDS, None, false);
    let mut rest = rest.iter();
    while let Some(flag) = rest.next() {
        if flag == "--dry-run" {
            dry_run = true;
            continue;
        }
        let value = rest.next().ok_or(usage)?;
        match flag.as_str() {
            "--max-rounds" => max_rounds = value.parse()?,
            "--only" if VARIANTS.iter().any(|(key, ..)| key == value) => {
                only = Some(value.clone());
            }
            _ => return Err(usage.into()),
        }
    }
    let (run, bench, out) = (Path::new(run), Path::new(bench), PathBuf::from(out));
    let set = QuestionSet::load(&bench.join("questions.yaml"))?;
    let reference = Reference::load(&bench.join("reference.yaml"))?;
    let mut matches = Matches::load(&bench.join("matches.yaml"))?;
    match Judgement::load(run) {
        Ok(judgement) => matches = matches.with_proposals(&judgement),
        Err(error) => eprintln!("no judge's pairs, the hand pairs only: {error}"),
    }
    let features = retrodoc_pipeline::load_features(run)
        .ok_or_else(|| format!("no features in {}", run.display()))?;
    let names: Vec<&str> = reference
        .domains
        .iter()
        .flat_map(|d| &d.features)
        .map(|f| f.name.as_str())
        .collect();
    let pairs = RunPairs::new(&matches, &features, &names);
    if dry_run {
        for question in &set.questions {
            println!("{}\t{:?}", question.id, pairs.group(question));
        }
        return Ok(());
    }
    let docs = Arc::new(load_docs(run)?);
    std::fs::create_dir_all(&out)?;

    let judge_tracker = UsageTracker::new();
    let judge = UsageProvider::new(provider(model, Some(0.0))?, judge_tracker.clone());
    let mut results = Vec::new();
    for (key, name, tools) in VARIANTS {
        if only.as_ref().is_some_and(|o| o != key) {
            continue;
        }
        let tracker = UsageTracker::new();
        let agent = UsageProvider::new(provider(model, None)?, tracker.clone());
        eprintln!("{name}: {} questions", set.questions.len());
        let records = ask_questions(&agent, &judge, &docs, &set, &pairs, tools, max_rounds).await;
        std::fs::write(
            out.join(format!("answers-{key}.yaml")),
            serde_yaml::to_string(&records)?,
        )?;
        results.push((name, records, tracker.report().total()));
    }
    let variants: Vec<Variant<'_>> = results
        .iter()
        .map(|(name, records, agent)| Variant {
            name,
            records,
            agent: *agent,
        })
        .collect();
    let table = format!(
        "Questions of {} on {} with {model}, {max_rounds} rounds at most.\n\n{}",
        bench.display(),
        run.display(),
        ask_table(&variants, &judge_tracker.report().total())
    );
    std::fs::write(out.join("table.md"), &table)?;
    print!("{table}");
    Ok(())
}

/// The docs `retrodoc ask` reads in `run`: the artifacts of its last `generate` and its collected
/// docs, the generated ones left out.
fn load_docs(run: &Path) -> Result<Docs, Failure> {
    let config = Config::load_ingest_and_output(run)?;
    let generated = config.output.generated_dirs();
    let existing: Vec<_> = retrodoc_ingest::existing_docs::load_existing_docs(
        run,
        &config.ingest.existing_docs_paths,
    )?
    .into_iter()
    .filter(|d| !generated.iter().any(|g| d.path.starts_with(g)))
    .collect();
    Docs::load(run, &existing)
        .ok_or_else(|| format!("no generated docs in {}", run.display()).into())
}

fn provider(model: &str, temperature: Option<f32>) -> Result<OpenRouterProvider, Failure> {
    let mut config: LlmConfig = serde_json::from_value(json!({
        "provider": "deepseek",
        "model": model,
        "timeout_secs": 600,
    }))?;
    config.temperature = temperature;
    Ok(OpenRouterProvider::from_config(&config)?)
}
