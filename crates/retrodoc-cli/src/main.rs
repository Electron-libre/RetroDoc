mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// RetroDoc — catch up on a project's documentation debt with AI agents.
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
    /// Builds the repo map (LLM summaries per file/module) and prints it.
    /// The rest of the pipeline (domains, features, ...) is not
    /// implemented yet — see PLAN.md §5.
    Generate {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Shows the documentation coverage report. Not implemented yet.
    Report {
        #[arg(long, default_value = ".")]
        path: PathBuf,
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
        Command::Generate { path } => commands::generate::run(&path).await,
        Command::Report { path } => commands::not_implemented("report", &path),
    }
}
