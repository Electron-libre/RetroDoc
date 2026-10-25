#!/usr/bin/env rust-script
//! Behavior test for `audit_sources.rs`: counts, typed-message filter, instruction files and memory.
//! Usage: `just test-harness`.

use std::path::Path;
use std::process::Command;

fn main() {
    let dir = std::fs::canonicalize(Path::new(file!()).parent().unwrap()).unwrap();
    let tmp = std::env::temp_dir().join(format!("audit_sources_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let repo = tmp.join("repo");
    let proj = tmp.join("projects/-tmp-repo");
    std::fs::create_dir_all(proj.join("s1/subagents")).unwrap();
    std::fs::create_dir_all(proj.join("memory")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("CLAUDE.md"), "# rules\n").unwrap();
    std::fs::write(proj.join("memory/feedback_x.md"), "memo\n").unwrap();
    std::fs::write(proj.join("memory/MEMORY.md"), "index\n").unwrap();
    let cwd = repo.display().to_string();
    let line = |t: &str, extra: &str, content: &str| format!("{{\"type\":\"{t}\",\"cwd\":{cwd:?}{extra},\"message\":{{\"content\":{content}}}}}\n");
    let transcript = [
        line("user", "", "\"never commit without asking\""),
        line("user", "", "[{\"type\":\"text\",\"text\":\"always run\\nthe tests\"}]"),
        line("user", "", "[{\"type\":\"tool_result\",\"content\":\"tool output\"}]"),
        line("user", ",\"isMeta\":true", "\"meta text\""),
        line("user", "", "\"<system-reminder>x</system-reminder>\""),
        line("user", "", "\"<div> is broken\\tat\\\\n\""),
        line("user", ",\"isSidechain\":true", "\"sidechain text\""),
        line("user", "", "\"This session is being continued from a previous conversation\""),
        line("assistant", "", "\"I answer\""),
    ]
    .concat();
    std::fs::write(proj.join("s1.jsonl"), transcript).unwrap();
    std::fs::write(proj.join("s1/subagents/a.jsonl"), line("user", "", "\"subagent prompt\"")).unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("rust-script")
            .arg(dir.join("audit_sources.rs"))
            .arg(tmp.join("projects"))
            .args(args)
            .output()
            .expect("rust-script must be installed");
        (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let mut failed = false;
    let mut check = |name: &str, ok: bool| {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    };
    let (ok, summary) = run(&[]);
    check("summary counts 1 session and 3 typed messages", ok && summary.contains("-tmp-repo: 1 sessions, 3 messages"));
    check("summary lists the instruction file of the project", summary.contains("CLAUDE.md"));
    check("summary lists the memory files", summary.contains("memory: 1 files"));
    let (_, messages) = run(&["--messages"]);
    check("typed messages are printed, newlines escaped", messages.contains("never commit without asking") && messages.contains("always run\\nthe tests"));
    check("a typed message starting with '<' is kept, tab and backslash escaped", messages.contains("<div> is broken\\tat\\\\n"));
    check("tool results, meta, reminders, assistant and subagents are skipped", ["tool output", "meta text", "system-reminder", "I answer", "subagent prompt", "sidechain text", "being continued"].iter().all(|s| !messages.contains(s)));

    let _ = std::fs::remove_dir_all(&tmp);
    if failed {
        std::process::exit(1);
    }
}
