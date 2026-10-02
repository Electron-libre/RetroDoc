mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .without_time()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Init { path, force } => commands::init::run(&path, force),
        Command::Scan { path } => commands::scan::run(&path),
        Command::Generate {
            path,
            dry_run,
            force,
        } => commands::generate::run(&path, dry_run, force).await,
        Command::Render { path, dry_run } => commands::render::run(&path, dry_run),
        Command::Report { path } => commands::report::run(&path),
        Command::Roles { path, force } => commands::roles::run(&path, force).await,
    }
}
