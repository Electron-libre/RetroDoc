//! `retrodoc chat`: questions read one per line, each answered like `ask` after the earlier ones
//! (`specs/chat.md`). Read-only: only the usage recap is saved, by the end of the command.

use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use retrodoc_agent::Conversation;
use retrodoc_llm::UsageTracker;

use super::docs::load_docs;
use super::passes;
use super::workspace::Workspace;

/// The lines that end the conversation, besides the end of the input.
const EXIT: [&str; 2] = ["exit", "quit"];

/// Answers each line of the standard input with the LLM settings of the `ask` pass, keeping the
/// conversation; a failed question is printed and the next one asked. Fails before any call when no
/// `generate` saved the docs.
pub async fn run(path: &Path, max_rounds: usize, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let docs = Arc::new(load_docs(&workspace)?);
    let llm = super::usage::pass_provider(&workspace.config, tracker, passes::ASK)?;
    println!("Ask a question about the application; `exit` or the end of the input ends the chat.");
    let mut conversation = Conversation::default();
    while let Some(line) = next_line().await? {
        let question = line.trim();
        if question.is_empty() {
            continue;
        }
        if EXIT.contains(&question) {
            break;
        }
        println!("Reading the docs to answer…");
        let answer = tracker
            .in_pass(
                passes::ASK,
                conversation.ask(llm.as_ref(), Arc::clone(&docs), question, max_rounds),
            )
            .await;
        match answer {
            Ok(answer) => println!("\n{}", answer.render()),
            Err(error) => println!("\nThe question could not be answered: {error}\n"),
        }
    }
    Ok(())
}

/// The next line of the standard input after a prompt, `None` at its end; read off the async
/// threads.
async fn next_line() -> anyhow::Result<Option<String>> {
    print!("> ");
    std::io::stdout().flush()?;
    tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        let read = std::io::stdin().read_line(&mut line)?;
        Ok::<_, std::io::Error>((read > 0).then_some(line))
    })
    .await?
    .context("the input could not be read")
}
