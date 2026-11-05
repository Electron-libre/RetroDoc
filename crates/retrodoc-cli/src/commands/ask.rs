//! `retrodoc ask`: the agent of `retrodoc-agent` answers a question from the generated docs
//! (`specs/ask.md`, ADR 0047). Read-only: only the usage recap is saved, by the end of the command.

use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use retrodoc_llm::UsageTracker;

use super::docs::load_docs;
use super::passes;
use super::workspace::Workspace;

/// Asks `question` with the LLM settings of the `ask` pass, then prints the answer and its sources.
/// Fails before any call when no `generate` saved the docs.
pub async fn run(
    path: &Path,
    question: &str,
    max_rounds: usize,
    tracker: &UsageTracker,
) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let docs = Arc::new(load_docs(&workspace)?);
    let llm = super::usage::pass_provider(&workspace.config, tracker, passes::ASK)?;
    println!("Reading the docs to answer…");
    let answer = tracker
        .in_pass(
            passes::ASK,
            retrodoc_agent::ask(llm.as_ref(), docs, question, max_rounds),
        )
        .await
        .context("the question could not be answered")?;
    println!("\n{}", answer.render());
    Ok(())
}
