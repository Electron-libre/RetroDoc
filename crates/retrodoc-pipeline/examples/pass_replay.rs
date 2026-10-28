//! Replays one pass alone on frozen inputs, N times, and scores each run against the hand-made reference
//! (`issues/done/benchmark_passes_on_fixed_inputs.md`).
//!
//! Usage:
//! `DEEPSEEK_API_KEY=x cargo run -q -p retrodoc-pipeline --example pass_replay -- <domains|features|use-cases|rules> <frozen dir> <benchmark dir> <model> [runs] [--freeze-first] [--brief off|head] [--evidence on|off]`
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
//! - `--brief off|head` (features, use cases): leave the frozen product brief out of the prompts, or put it
//!   at the head of the user prompt (the default, as `generate`).
//! - `--evidence on|off` (features, use cases): give each unit the extracts of the signals closest to it,
//!   from the tree at `manifest.source_run`, or not (`brief.evidence`; `on`, the default, as `generate`).
//!
//! Each run starts from an empty work directory and the temperature is the pipeline's for the pass. The
//! same model then judges what the strict reading cannot (the "judge" columns: an LLM's reading, not the
//! strict one). Prints one Markdown table.

use std::path::{Path, PathBuf};

use retrodoc_core::config::{LlmConfig, ZERO_TEMPERATURE_PASSES};
use retrodoc_llm::{OpenRouterProvider, UsageProvider, UsageTracker};
use retrodoc_pipeline::benchmark::{
    replay_domains, replay_features, replay_rules, replay_table, replay_use_cases, rule_table,
    tree_evidence, use_case_table, Frame, FrozenInputs, Judgement, Matches, Reference, RunFigures,
    Scoring,
};
use retrodoc_pipeline::Artifact;
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
    frame: Frame,
    /// How the frame was set, for the title.
    frame_label: String,
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
            "{} of {} with {}{}",
            self.pass, self.frozen.manifest.repo, self.model, self.frame_label
        )
    }

    fn origin(&self) -> String {
        format!(
            "written from the surface by {}{}, first run of pass_replay",
            self.model, self.frame_label
        )
    }
}

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let options = match Options::take(&mut args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let [pass, frozen_dir, bench_dir, model, rest @ ..] = args.as_slice() else {
        eprintln!("usage: pass_replay <domains|features|use-cases|rules> <frozen dir> <benchmark dir> <model> [runs] [--freeze-first] [--brief off|head] [--evidence on|off]");
        std::process::exit(2);
    };
    let runs = rest.first().and_then(|n| n.parse().ok()).unwrap_or(3);
    let result = start(pass, frozen_dir, bench_dir, model, runs, options).await;
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

/// The flags of the command line.
struct Options {
    freeze_first: bool,
    without_brief: bool,
    evidence: bool,
}

impl Default for Options {
    /// The frame `generate` gives by default: the brief and the evidence.
    fn default() -> Self {
        Self {
            freeze_first: false,
            without_brief: false,
            evidence: true,
        }
    }
}

impl Options {
    /// Takes the flags out of `args`, leaving the positional arguments.
    fn take(args: &mut Vec<String>) -> Result<Self, Failure> {
        let mut options = Options::default();
        let mut rest = Vec::new();
        let mut given = std::mem::take(args).into_iter();
        while let Some(arg) = given.next() {
            match arg.as_str() {
                "--freeze-first" => options.freeze_first = true,
                "--evidence" => match given.next().as_deref() {
                    Some("on") => options.evidence = true,
                    Some("off") => options.evidence = false,
                    _ => return Err("--evidence takes `on` or `off`".into()),
                },
                "--brief" => match given.next().as_deref() {
                    Some("off") => options.without_brief = true,
                    Some("head") => options.without_brief = false,
                    _ => return Err("--brief takes `off` or `head`".into()),
                },
                flag if flag.starts_with("--") => {
                    return Err(format!("unknown option `{flag}`").into());
                }
                _ => rest.push(arg),
            }
        }
        *args = rest;
        Ok(options)
    }

    /// The frame of the replayed pass, read from the tree when it takes the evidence.
    fn frame(&self, tree: &Path) -> Result<Frame, Failure> {
        let evidence = if self.evidence {
            tree_evidence(tree)?
        } else {
            retrodoc_pipeline::Evidence::default()
        };
        Ok(Frame {
            without_brief: self.without_brief,
            evidence,
        })
    }

    /// How the frame was set, said in every title: the replays of the features and use cases had no
    /// evidence before it was on by default, so a title never leaves it implicit.
    fn frame_label(&self) -> String {
        let brief = if self.without_brief {
            "no brief"
        } else {
            "brief"
        };
        let evidence = if self.evidence {
            "evidence"
        } else {
            "no evidence"
        };
        format!(", {brief}, {evidence}")
    }
}

/// Warns when the tree the evidence is read from is no longer at the commit of the frozen inputs: its
/// docs, tests and commits would not be those the frozen run saw.
fn warn_if_moved(tree: &Path, commit: &str) {
    let head = git2::Repository::open(tree)
        .and_then(|repo| Ok(repo.head()?.peel_to_commit()?.id().to_string()));
    match head {
        Ok(head) if head == commit => {}
        Ok(head) => eprintln!(
            "warning: {} is at {head}, the frozen inputs at {commit}: the evidence comes from another commit",
            tree.display()
        ),
        Err(error) => eprintln!("warning: commit of {} unknown: {error}", tree.display()),
    }
}

async fn start(
    pass: &str,
    frozen_dir: &str,
    bench_dir: &str,
    model: &str,
    runs: usize,
    options: Options,
) -> Result<(), Failure> {
    if !["domains", "features", "use-cases", "rules"].contains(&pass)
        || (options.freeze_first && pass == "rules")
    {
        return Err("pass is `domains`, `features`, `use-cases` or `rules`; --freeze-first goes with the first three".into());
    }
    let framed = ["features", "use-cases"].contains(&pass);
    if !framed && (options.without_brief || !options.evidence) {
        return Err("--brief off and --evidence off go with `features` and `use-cases`".into());
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
    let frozen = FrozenInputs::open(Path::new(frozen_dir))?;
    let frame = if framed {
        let tree = Path::new(&frozen.manifest.source_run);
        warn_if_moved(tree, &frozen.manifest.commit);
        options.frame(tree)?
    } else {
        Frame::default()
    };
    let mut setup = Setup {
        pass: pass.to_string(),
        model: model.to_string(),
        runs,
        freeze_first: options.freeze_first,
        frozen,
        reference: Reference::load(&bench_dir.join("reference.yaml"))?,
        matches: Matches::load(&matches_path)?,
        matches_path,
        config,
        frame,
        frame_label: if framed {
            options.frame_label()
        } else {
            String::new()
        },
    };
    match pass {
        "use-cases" => use_cases(&mut setup).await,
        "rules" => rules(&setup).await,
        _ => surface_pass(&mut setup).await,
    }
}

/// Copies the `artifact` of a run to the directory the environment variable `var` names, as `name`,
/// when it is set. A run that wrote no such file has nothing to keep.
fn keep(work: &Path, artifact: Artifact, var: &str, name: &str) -> Result<(), Failure> {
    keep_file(&artifact.path(work), var, name)
}

/// The same for any file of the run.
fn keep_file(from: &Path, var: &str, name: &str) -> Result<(), Failure> {
    let Some(dir) = std::env::var_os(var) else {
        return Ok(());
    };
    std::fs::create_dir_all(&dir)?;
    if from.exists() {
        std::fs::copy(from, Path::new(&dir).join(name))?;
    }
    Ok(())
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
        let total = tracker.report().total();
        // RETRODOC_KEEP_RULES=<dir>: keep the rules of each run to read them (`rules-<n>.yaml`).
        keep(
            work.path(),
            Artifact::Rules,
            "RETRODOC_KEEP_RULES",
            &format!("rules-{n}.yaml"),
        )?;
        eprintln!(
            "run {n}/{}: {} rules, {} prompt tokens of which {} cached",
            setup.runs, run.published, total.prompt_tokens, total.cached_tokens
        );
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
            &setup.frame,
        )
        .await?;
        if setup.freeze_first && n == 1 {
            let origin = setup.origin();
            setup.frozen.replace_use_cases(work.path(), &origin)?;
        }
        // RETRODOC_KEEP_USE_CASES=<dir>: keep the use cases of each run (`use-cases-<n>.yaml`).
        keep(
            work.path(),
            Artifact::UseCases,
            "RETRODOC_KEEP_USE_CASES",
            &format!("use-cases-{n}.yaml"),
        )?;
        let total = tracker.report().total();
        eprintln!(
            "run {n}/{}: {} use cases, {} prompt tokens of which {} cached",
            setup.runs, run.use_cases, total.prompt_tokens, total.cached_tokens
        );
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
                &setup.frame,
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
        // RETRODOC_KEEP_FEATURES=<dir>: keep the result and the judgement of each run to audit them
        // (`<pass>-<n>.yaml`, `judge-<n>.yaml`).
        let result = if setup.pass == "domains" {
            Artifact::Domains
        } else {
            Artifact::Features
        };
        let var = "RETRODOC_KEEP_FEATURES";
        keep(
            work.path(),
            result,
            var,
            &format!("{}-{n}.yaml", setup.pass),
        )?;
        keep_file(
            &Judgement::path(work.path()),
            var,
            &format!("judge-{n}.yaml"),
        )?;
        eprintln!("run {n}/{}: {} {}", setup.runs, run.items, setup.pass);
        figures.push(run);
    }
    println!("{}", setup.head());
    println!("{}", replay_table(&setup.title(), &figures));
    Ok(())
}
