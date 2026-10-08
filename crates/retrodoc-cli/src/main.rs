mod commands;

use std::io::IsTerminal;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use retrodoc_llm::UsageTracker;
use tracing_subscriber::fmt::writer::BoxMakeWriter;

/// `RetroDoc` — catch up on a project's documentation debt with AI agents.
#[derive(Debug, Parser)]
#[command(name = "retrodoc", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Creates a default `retrodoc.toml` at the repo root.
    Init {
        /// Root of the repo to document (default: current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Overwrite an existing `retrodoc.toml`.
        #[arg(long)]
        force: bool,
    },
    /// Walks the repo (files + git history + existing Markdown docs) and
    /// prints a summary, without writing anything to disk.
    Scan {
        /// Root of the repo to document (default: current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Runs the pipeline (repo map, domains, features, use cases,
    /// confidence), saves the intermediate artifacts under
    /// `.retrodoc/cache/` and writes the docs. Units unchanged since the
    /// last run are not sent to the LLM again.
    Generate {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Preview the docs (files and diffs) without writing them. The
        /// intermediate artifacts in `.retrodoc/cache/` are still updated.
        #[arg(long)]
        dry_run: bool,
        /// Ignore the caches and redo every LLM pass from scratch.
        #[arg(long)]
        force: bool,
        /// Skip the confidence pass (about a third of the LLM calls): use
        /// cases and features stay unscored.
        #[arg(long, conflicts_with = "confidence_sample")]
        no_confidence: bool,
        /// Score at most N use cases not scored yet, evenly spread; the
        /// others stay unscored (a later run scores N more).
        #[arg(long, value_name = "N")]
        confidence_sample: Option<usize>,
        /// Analyse at most N source files: the best ranked by role, git
        /// history and references; the rest is listed in the report.
        /// Overrides `ingest.max_files`.
        #[arg(long, value_name = "N")]
        max_files: Option<usize>,
    },
    /// Writes the docs from the artifacts of the last `generate` run,
    /// without calling the LLM.
    Render {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Preview the docs (files and diffs) without writing them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Shows the documentation debt report (confidence per domain, weak
    /// sections) from the last `generate` run, without calling the LLM.
    Report {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Identifies the business actors (who uses the application, in business
    /// terms) from the authorization code and the entities, and
    /// saves `.retrodoc/cache/actors.yaml`. Uses the glossary and entry
    /// points of the earlier commands when present.
    Actors {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Ignore the saved list and identify the actors again.
        #[arg(long)]
        force: bool,
    },
    /// Measures the docs of the last `generate` run against a hand-written
    /// reference: sizes, business-language and confidence scores, cost, and
    /// recall and precision of domains and features. No LLM call unless
    /// `--judge`, which asks the model to pair what the names alone leave
    /// unmatched and to rate the narratives (see `issues/quality_benchmark.md`).
    Benchmark {
        /// Repository whose last run is measured (a clone, not the original).
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// The reference, e.g. `benchmark/<repo>/reference.yaml`.
        #[arg(long)]
        reference: PathBuf,
        /// The hand-written pairs (default: `matches.yaml` next to the reference).
        #[arg(long)]
        matches: Option<PathBuf>,
        /// Also call the LLM judge.
        #[arg(long)]
        judge: bool,
        /// Also write the figures of the run as JSON (for `benchmark-table`).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Prints the table that compares benchmark runs: mean, range and count
    /// of each figure per series (e.g. user docs hidden or shown), and the
    /// change against a previous benchmark. Reads `<dir>/<series>/*.json`
    /// written by `retrodoc benchmark --out`. No LLM call.
    BenchmarkTable {
        dir: PathBuf,
        /// A previous benchmark directory, to show the change.
        #[arg(long)]
        previous: Option<PathBuf>,
    },
    /// Searches the generated documentation (domains, features, use cases,
    /// glossary) and the collected docs, lexically, from the artifacts of the
    /// last `generate` run. No LLM call: shows what the MCP server will find.
    Search {
        /// What to look for, in the application's own words.
        query: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Maximum number of results.
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Serves the generated documentation to LLM agents (Claude Code, Cursor…)
    /// as an MCP server over stdin/stdout. Read-only, no LLM call, needs a
    /// `generate` run. Configure the agent to launch `retrodoc mcp --path
    /// <repo>`.
    Mcp {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Prints the application surface (entities, entry points by resource)
    /// as the domain clustering receives it. No LLM call; needs the
    /// artifacts of `retrodoc glossary` and `retrodoc entry-points`.
    Surface {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Identifies the stack and assigns a role (entrypoint, model, logic…)
    /// to every file, from one LLM call over the file tree. The rules are
    /// saved in `.retrodoc/cache/roles.yaml` and can be edited by hand.
    Roles {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Ignore the saved rules and identify them again.
        #[arg(long)]
        force: bool,
    },
    /// Writes the product brief (what the application does, for whom, its
    /// main business objects and capabilities) from the non-code evidence:
    /// docs, commits, manifests, schema, translations, tests. Saved in
    /// `.retrodoc/cache/product.yaml`, editable by hand.
    Brief {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Write the brief again (and locate the schema and translations
        /// again), edits included.
        #[arg(long)]
        force: bool,
        /// Only print how much evidence the repository gives, per kind. No
        /// LLM call.
        #[arg(long)]
        signals: bool,
    },
    /// Locates where the business lives (the files and directories that
    /// hold the domain data and rules) from one LLM call over the tree, the
    /// stack, the product brief and cheap evidence. Saved in
    /// `.retrodoc/cache/business-files.yaml`, editable by hand.
    BusinessFiles {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Ask again, edits included.
        #[arg(long)]
        force: bool,
    },
    /// Reads the entry points (routes, commands, jobs, public API…) and
    /// their outputs from the files classified `entrypoint`, and saves
    /// `.retrodoc/cache/entry-points.yaml`. Needs `retrodoc roles` first.
    EntryPoints {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Reads the business entities (names, attributes, associations) of the
    /// files classified `model` and the vocabulary of the tests, and saves
    /// `.retrodoc/cache/glossary.yaml`. Needs `retrodoc roles` first.
    Glossary {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
}

impl Command {
    /// The commands that call the LLM, with the name and repo path their
    /// usage recap is filed under.
    fn counted(&self) -> Option<(&'static str, PathBuf)> {
        match self {
            Command::Generate { path, .. } => Some(("generate", path.clone())),
            Command::Roles { path, .. } => Some(("roles", path.clone())),
            Command::Brief { path, .. } => Some(("brief", path.clone())),
            Command::BusinessFiles { path, .. } => Some(("business-files", path.clone())),
            Command::Glossary { path } => Some(("glossary", path.clone())),
            Command::EntryPoints { path } => Some(("entry-points", path.clone())),
            Command::Actors { path, .. } => Some(("actors", path.clone())),
            Command::Benchmark {
                path, judge: true, ..
            } => Some(("benchmark", path.clone())),
            _ => None,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // API keys may live in a `.env` (current directory or a parent); variables already set win.
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();
    // The logs go to stdout (plain text when redirected to a file or a pipe: saved logs, smoke
    // test), except for `mcp`, whose stdout carries the protocol.
    let logs_to_stderr = matches!(cli.command, Command::Mcp { .. });
    let ansi = if logs_to_stderr {
        std::io::stderr().is_terminal()
    } else {
        std::io::stdout().is_terminal()
    };
    let writer = if logs_to_stderr {
        BoxMakeWriter::new(std::io::stderr)
    } else {
        BoxMakeWriter::new(std::io::stdout)
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .without_time()
        .with_writer(writer)
        .with_ansi(ansi)
        .init();

    let tracker = UsageTracker::new();
    let counted = cli.command.counted();
    let result = match cli.command {
        Command::Init { path, force } => commands::init::run(&path, force),
        Command::Scan { path } => commands::scan::run(&path),
        Command::Generate {
            path,
            dry_run,
            force,
            no_confidence,
            confidence_sample,
            max_files,
        } => {
            let confidence = if no_confidence {
                commands::generate::Confidence::Skip
            } else {
                commands::generate::Confidence::Sample(confidence_sample)
            };
            let options = commands::generate::GenerateOptions {
                dry_run,
                force,
                confidence,
                max_files,
            };
            commands::generate::run(&path, &options, &tracker).await
        }
        Command::Render { path, dry_run } => commands::render::run(&path, dry_run),
        Command::Report { path } => commands::report::run(&path),
        Command::EntryPoints { path } => commands::entry_points::run(&path, &tracker).await,
        Command::Glossary { path } => commands::glossary::run(&path, &tracker).await,
        Command::Actors { path, force } => commands::actors::run(&path, force, &tracker).await,
        Command::Benchmark {
            path,
            reference,
            matches,
            judge,
            out,
        } => {
            let options = commands::benchmark::BenchmarkOptions {
                reference,
                matches,
                judge,
                out,
            };
            commands::benchmark::run(&path, &options, &tracker).await
        }
        Command::BenchmarkTable { dir, previous } => {
            commands::benchmark::table(&dir, previous.as_deref())
        }
        Command::Mcp { path } => commands::mcp::run(&path).await,
        Command::Search { query, path, limit } => commands::search::run(&path, &query, limit),
        Command::Surface { path } => commands::surface::run(&path),
        Command::Roles { path, force } => commands::roles::run(&path, force, &tracker).await,
        Command::Brief {
            path,
            force,
            signals,
        } => commands::brief::run(&path, force, signals, &tracker).await,
        Command::BusinessFiles { path, force } => {
            commands::business_files::run(&path, force, &tracker).await
        }
    };
    if let Some((name, path)) = counted {
        commands::usage::finish(&path, name, &tracker, result.is_ok());
    }
    result
}
