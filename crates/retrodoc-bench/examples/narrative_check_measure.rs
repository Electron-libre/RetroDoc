//! Runs the confidence pass on a sample of saved use cases and prints how it judged their narratives
//! (`issues/done/narrative_checked_against_cited_code.md`, deliverable 3), to compare with the hand labels.
//!
//! Usage:
//! `DEEPSEEK_API_KEY=x cargo run -q -p retrodoc-pipeline --example narrative_check_measure -- <code tree> <use-cases.yaml> <work dir> <stride> <model> [--all]`
//!
//! - `code tree`: the checkout the use cases cite, with its `.retrodoc/cache/glossary.yaml`. Never written:
//!   it is copied (without `.git` and `.retrodoc`, the glossary put back) to `work dir`, which must not exist.
//! - `use-cases.yaml`: the saved use cases; the 1st, then every `stride`-th, are scored.
//! - `model`: a `DeepSeek` model, e.g. `deepseek-chat`.
//! - `--all`: the sampled use cases whose steps all pass the checks go to the model too
//!   (`--confidence-sample` covering them all), not only those with a doubtful step as in `generate`; one
//!   with no doubtful step and a step the checks fail, or citing no readable code, still does not.
//!
//! Prints one Markdown row per use case (sent to the model or not, score, narrative verdict and reason)
//! and the calls and tokens of the pass.

use std::path::Path;

use retrodoc_core::config::LlmConfig;
use retrodoc_core::model::UseCase;
use retrodoc_llm::{OpenRouterProvider, UsageProvider, UsageTracker};
use retrodoc_pipeline::{score_confidence, WalkedFiles};
use serde_json::json;

type Failure = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let all = args.iter().any(|a| a == "--all");
    args.retain(|a| a != "--all");
    let [tree, use_cases, work, stride, model] = args.as_slice() else {
        eprintln!("usage: narrative_check_measure <code tree> <use-cases.yaml> <work dir> <stride> <model> [--all]");
        std::process::exit(2);
    };
    let Ok(stride) = stride.parse::<usize>() else {
        eprintln!("stride must be a number");
        std::process::exit(2);
    };
    let tree = Path::new(tree);
    let work = Path::new(work);
    if let Err(error) = run(tree, Path::new(use_cases), work, stride.max(1), model, all).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run(
    tree: &Path,
    use_cases: &Path,
    work: &Path,
    stride: usize,
    model: &str,
    all: bool,
) -> Result<(), Failure> {
    if work.exists() {
        return Err(format!("{} exists: give a new work dir", work.display()).into());
    }
    copy_tree(tree, work)?;
    let cache = work.join(".retrodoc/cache");
    std::fs::create_dir_all(&cache)?;
    let glossary = tree.join(".retrodoc/cache/glossary.yaml");
    if glossary.exists() {
        std::fs::copy(&glossary, cache.join("glossary.yaml"))?;
    }
    let saved: Vec<UseCase> = serde_yaml::from_str(&std::fs::read_to_string(use_cases)?)?;
    let mut sample: Vec<UseCase> = saved
        .into_iter()
        .step_by(stride)
        .map(|mut u| {
            u.confidence = None;
            u.narrative_check = None;
            u
        })
        .collect();

    let mut config: LlmConfig = serde_json::from_value(json!({
        "provider": "deepseek",
        "model": model,
        "timeout_secs": 600,
    }))?;
    config.temperature = Some(0.0);
    let tracker = UsageTracker::new();
    let llm = UsageProvider::new(OpenRouterProvider::from_config(&config)?, tracker.clone());
    let walked = WalkedFiles::walk(work, &[])?;
    let count = sample.len();
    tracker.set_pass("confidence");
    score_confidence(&walked, &mut [], &mut sample, &llm, all.then_some(count), 4).await?;
    tracker.end_pass();

    println!(
        "| use case | narrative | score | narrative verdict | reason |\n|---|---|---|---|---|"
    );
    for u in &sample {
        let narrated = u.narrative.as_deref().is_some_and(|n| !n.trim().is_empty());
        let (verdict, reason) = u.narrative_check.as_ref().map_or_else(
            || ("not checked".to_string(), String::new()),
            |c| {
                (
                    format!("{:.1}", c.value),
                    cell(c.rationale.as_deref().unwrap_or_default()),
                )
            },
        );
        let score = u
            .confidence
            .as_ref()
            .map_or_else(|| "unscored".to_string(), |c| format!("{:.2}", c.value));
        println!(
            "| {} | {} | {score} | {verdict} | {reason} |",
            u.slug,
            if narrated { "yes" } else { "no" },
        );
    }
    let total = tracker.report().total();
    println!(
        "\n{count} use cases, {} calls, {} prompt tokens, {} completion tokens",
        total.calls, total.prompt_tokens, total.completion_tokens
    );
    Ok(())
}

/// `text` fit for a Markdown table cell: one line, `|` escaped.
fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

/// Copies `from` into `to`, without `.git` and `.retrodoc`.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == ".retrodoc" {
            continue;
        }
        let target = to.join(&name);
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
