#!/usr/bin/env rust-script
//! List the sources of the instruction audit: per project of the transcripts directory, the sessions, the
//! messages typed by the user, the memory files and the instruction files (`CLAUDE.md`, `AGENTS.md`) of its
//! working directory. With `--messages`, print the typed messages as `project<TAB>session<TAB>text`
//! (newlines escaped); subagent transcripts are skipped. Prints private content: never commit the output.
//! Usage: `rust-script audit_sources.rs [projects_dir] [--messages]` (default `~/.claude/projects`).
//! ```cargo
//! [dependencies]
//! serde_json = "1"
//! ```

use serde_json::Value;
use std::path::{Path, PathBuf};

/// One line per message: tab, carriage return and backslash are escaped along with the newline.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")
}

const INSTRUCTION_FILES: [&str; 2] = ["CLAUDE.md", "AGENTS.md"];

/// The text a user typed in a transcript line; `None` for tool results, meta and sidechain lines, injected text and other types.
fn typed_text(line: &Value) -> Option<String> {
    if line["type"] != "user" || line["isMeta"].as_bool() == Some(true) || line["isSidechain"].as_bool() == Some(true) {
        return None;
    }
    let content = &line["message"]["content"];
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter(|p| p["type"] == "text").filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => return None,
    };
    let t = text.trim();
    // Reminders, command echoes and the compaction summary are injected as user lines; a typed `<div> broken` is not.
    let injected = (t.starts_with('<') && t.ends_with('>')) || t.starts_with("This session is being continued");
    (!t.is_empty() && !injected).then_some(text)
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    paths.sort();
    paths
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let print_messages = args.iter().any(|a| a == "--messages");
    if let Some(flag) = args.iter().find(|a| a.starts_with('-') && *a != "--messages") {
        eprintln!("unknown option {flag}\nusage: audit_sources.rs [projects_dir] [--messages]");
        std::process::exit(2);
    }
    let root = match args.iter().find(|a| !a.starts_with('-')) {
        Some(dir) => PathBuf::from(dir),
        None => match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home).join(".claude/projects"),
            Err(_) => {
                eprintln!("HOME is not set: pass the projects directory");
                std::process::exit(2);
            }
        },
    };
    for project in sorted_entries(&root).into_iter().filter(|p| p.is_dir()) {
        let name = project.file_name().unwrap().to_string_lossy().into_owned();
        let (mut sessions, mut messages, mut cwds) = (0, 0, Vec::<String>::new());
        for session in sorted_entries(&project).into_iter().filter(|p| p.extension().is_some_and(|e| e == "jsonl")) {
            sessions += 1;
            let stem = session.file_stem().unwrap().to_string_lossy().into_owned();
            let text = match std::fs::read_to_string(&session) {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("cannot read {}: {e}", session.display());
                    continue;
                }
            };
            let mut skipped = 0;
            for line in text.lines().filter(|l| !l.is_empty()) {
                let Ok(line) = serde_json::from_str::<Value>(line) else {
                    skipped += 1;
                    continue;
                };
                if let Some(c) = line["cwd"].as_str().filter(|c| !cwds.iter().any(|k| k == c)) {
                    cwds.push(c.to_string());
                }
                if let Some(t) = typed_text(&line) {
                    messages += 1;
                    if print_messages {
                        println!("{name}\t{stem}\t{}", escape(&t));
                    }
                }
            }
            if skipped > 0 {
                eprintln!("{}: {skipped} unreadable lines skipped", session.display());
            }
        }
        if print_messages {
            continue;
        }
        println!("{name}: {sessions} sessions, {messages} messages");
        let is_note = |p: &PathBuf| p.extension().is_some_and(|e| e == "md") && p.file_name().is_some_and(|n| n != "MEMORY.md");
        let memory = sorted_entries(&project.join("memory")).iter().filter(|p| is_note(p)).count();
        if memory > 0 {
            println!("  memory: {memory} files");
        }
        for cwd in &cwds {
            for f in INSTRUCTION_FILES.iter().filter(|f| Path::new(cwd).join(f).is_file()) {
                println!("  instructions: {cwd}/{f}");
            }
        }
    }
}
