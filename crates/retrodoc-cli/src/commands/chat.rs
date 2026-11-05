//! `retrodoc chat`: questions read one per line, each answered like `ask` after the earlier ones
//! (`specs/chat.md`). Read-only: only the usage recap is saved, by the end of the command, which
//! an interrupt reaches too (CHT-10).

use std::io::{BufRead as _, Write as _};
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
/// conversation; a failed question is printed and the next one asked; an interrupt ends it like the
/// end of the input. Fails before any call when no `generate` saved the docs.
pub async fn run(path: &Path, max_rounds: usize, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let docs = Arc::new(load_docs(&workspace)?);
    let llm = super::usage::pass_provider(&workspace.config, tracker, passes::ASK)?;
    println!("Ask a question about the application; `exit` or the end of the input ends the chat.");
    let mut conversation = Conversation::default();
    let mut lines = input_lines();
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    loop {
        prompt()?;
        let line = tokio::select! {
            line = lines.recv() => line,
            _ = &mut interrupt => break,
        };
        let Some(line) = line else { break };
        let line = line.context("the input could not be read")?;
        let question = line.trim();
        if question.is_empty() {
            continue;
        }
        if EXIT.iter().any(|word| question.eq_ignore_ascii_case(word)) {
            break;
        }
        println!("Reading the docs to answer…");
        let asking = tracker.in_pass(
            passes::ASK,
            conversation.ask(llm.as_ref(), Arc::clone(&docs), question, max_rounds),
        );
        let answer = tokio::select! {
            answer = asking => answer,
            _ = &mut interrupt => break,
        };
        match answer {
            Ok(answer) => println!("\n{}", answer.render()),
            Err(error) => println!("\nThe question could not be answered: {error}\n"),
        }
    }
    println!();
    Ok(())
}

/// The lines of the standard input, read by a thread of their own, a byte that is not UTF-8
/// replaced; the channel closes at the end of the input. Not a blocking task of the runtime, whose
/// shutdown would wait for the read an interrupt leaves pending.
fn input_lines() -> tokio::sync::mpsc::Receiver<std::io::Result<String>> {
    let (send, receive) = tokio::sync::mpsc::channel(1);
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        loop {
            let mut line = Vec::new();
            let read = match stdin.read_until(b'\n', &mut line) {
                Ok(0) => return,
                Ok(_) => Ok(String::from_utf8_lossy(&line).into_owned()),
                Err(error) => Err(error),
            };
            if send.blocking_send(read).is_err() {
                return;
            }
        }
    });
    receive
}

/// Prints the prompt of the next question.
fn prompt() -> anyhow::Result<()> {
    print!("> ");
    std::io::stdout().flush()?;
    Ok(())
}
