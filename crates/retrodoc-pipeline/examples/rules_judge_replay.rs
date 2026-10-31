//! Replays the judgement of the rules on the rules saved runs stated, N times, and scores it on hand
//! labels (`issues/rules_plumbing_and_screen_noise.md`): what a variant of the judgement drops on top of
//! the judgement of the run, with the statements fixed.
//!
//! Usage:
//! `DEEPSEEK_API_KEY=x cargo run -q -p retrodoc-pipeline --example rules_judge_replay -- <model> [runs] [--prompt <file>] [--labels <dir>] [--keep <dir>] [--neighbours] <run dir>...`
//!
//! - `model`: a `DeepSeek` model, e.g. `deepseek-chat`; temperature 0, as the rules pass. `runs`: default 3.
//! - `--prompt <file>`: the system prompt of the judgement (default: the pass's own).
//! - `--labels <dir>`: `<dir>/<name of the run dir>.tsv`, one `<rule slug>\t<label>` a line, `good` or a
//!   kind of noise (`plumbing`, `screen`, `test`, `duplicate`): adds the good rules and the noise dropped.
//! - `--neighbours`: shows each feature the rules of the other features stated on the same files.
//! - `--keep <dir>`: writes the class of each rule as `<dir>/<name of the run dir>-<n>.tsv`.
//! - `run dir`: a checkout with its `.retrodoc/cache` (`features.yaml`, `rules-stated.yaml`).
//!
//! Prints one Markdown table, a line per run dir and replay.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_core::config::LlmConfig;
use retrodoc_core::model::BusinessRule;
use retrodoc_llm::{OpenRouterProvider, UsageProvider, UsageTracker};
use retrodoc_pipeline::benchmark::{parse_labels, replay_judgement, score_labels, LabelScore};
use retrodoc_pipeline::rules::{JUDGE_SYSTEM_PROMPT, NOISE};
use retrodoc_pipeline::{load_features, Artifact};
use serde_json::json;

type Failure = Box<dyn std::error::Error>;

/// Features judged at once.
const CONCURRENCY: usize = 8;

struct Options {
    model: String,
    runs: usize,
    prompt: String,
    labels: Option<PathBuf>,
    keep: Option<PathBuf>,
    neighbours: bool,
    dirs: Vec<PathBuf>,
}

fn options() -> Result<Options, Failure> {
    let mut args = std::env::args().skip(1);
    let model = args.next().ok_or("no model")?;
    let mut options = Options {
        model,
        runs: 3,
        prompt: JUDGE_SYSTEM_PROMPT.to_string(),
        labels: None,
        keep: None,
        neighbours: false,
        dirs: Vec::new(),
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--prompt" => {
                options.prompt = std::fs::read_to_string(args.next().ok_or("--prompt <file>")?)?;
            }
            "--labels" => options.labels = Some(args.next().ok_or("--labels <dir>")?.into()),
            "--neighbours" => options.neighbours = true,
            "--keep" => options.keep = Some(args.next().ok_or("--keep <dir>")?.into()),
            _ if options.dirs.is_empty() && arg.parse::<usize>().is_ok() => {
                options.runs = arg.parse()?;
            }
            _ => options.dirs.push(arg.into()),
        }
    }
    if options.dirs.is_empty() {
        return Err("no run dir".into());
    }
    let names: std::collections::BTreeSet<String> = options.dirs.iter().map(|d| name(d)).collect();
    if names.len() < options.dirs.len() {
        return Err("two run dirs share a name: their labels and kept classes would mix".into());
    }
    Ok(options)
}

fn name(dir: &Path) -> String {
    dir.file_name().map_or_else(
        || dir.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn share(part: usize, whole: usize) -> String {
    if whole == 0 {
        "-".to_string()
    } else {
        format!("{part}/{whole}")
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("usage: rules_judge_replay <model> [runs] [--prompt <file>] [--labels <dir>] [--keep <dir>] [--neighbours] <run dir>...\n{error}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), Failure> {
    let options = options()?;
    let mut config: LlmConfig = serde_json::from_value(json!({
        "provider": "deepseek",
        "model": options.model,
        "timeout_secs": 600,
    }))?;
    config.temperature = Some(0.0);
    if let Some(keep) = &options.keep {
        std::fs::create_dir_all(keep)?;
    }
    let mut table = String::from(
        "| run | replay | rules | dropped | storage | screen | test | technical | unread | good dropped | noise dropped | calls | prompt tokens |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for dir in &options.dirs {
        let features =
            load_features(dir).ok_or_else(|| format!("{}: no features.yaml", dir.display()))?;
        let rules: Vec<BusinessRule> =
            serde_yaml::from_str(&std::fs::read_to_string(Artifact::RulesStated.path(dir))?)?;
        let labels = match &options.labels {
            Some(labels) => {
                let path = labels.join(format!("{}.tsv", name(dir)));
                match std::fs::read_to_string(&path) {
                    Ok(text) => parse_labels(&text),
                    Err(err) => {
                        eprintln!("{}: no labels read ({err}), not scored", path.display());
                        BTreeMap::new()
                    }
                }
            }
            None => BTreeMap::new(),
        };
        for n in 1..=options.runs {
            let tracker = UsageTracker::new();
            let llm =
                UsageProvider::new(OpenRouterProvider::from_config(&config)?, tracker.clone());
            let figures = replay_judgement(
                &llm,
                &options.prompt,
                &features,
                &rules,
                options.neighbours,
                CONCURRENCY,
            )
            .await?;
            let total = tracker.report().total();
            let dropped = figures.dropped_by_class();
            let score = score_labels(&figures, &labels);
            if score.missing > 0 {
                eprintln!(
                    "{}: {} labels name no rule judged",
                    name(dir),
                    score.missing
                );
            }
            let LabelScore {
                good,
                good_dropped,
                noise,
                noise_dropped,
                ..
            } = score;
            let _ = write!(
                table,
                "| {} | {n} | {} | {} |",
                name(dir),
                figures.classes.len(),
                dropped.values().sum::<usize>()
            );
            for class in NOISE {
                let _ = write!(table, " {} |", dropped.get(class).unwrap_or(&0));
            }
            let _ = writeln!(
                table,
                " {} | {} | {} | {} | {} |",
                figures.unread,
                share(good_dropped, good),
                share(noise_dropped, noise),
                total.calls,
                total.prompt_tokens
            );
            if let Some(keep) = &options.keep {
                let mut classes = String::new();
                for (slug, class) in &figures.classes {
                    let _ = writeln!(classes, "{slug}\t{class}");
                }
                std::fs::write(keep.join(format!("{}-{n}.tsv", name(dir))), classes)?;
            }
            eprintln!("{} replay {n}/{} done", name(dir), options.runs);
        }
    }
    print!("{table}");
    Ok(())
}
