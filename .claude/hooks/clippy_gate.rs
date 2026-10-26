#!/usr/bin/env rust-script
//! Stop hook: if Rust files changed, clippy must be warning-free before the agent can finish.
//! Exit 2 sends clippy's diagnostics back to the agent so it fixes the code: one line each (file, line, column,
//! message), at most `MAX_DIAGNOSTICS`, since every line is billed to the agent. A passing run stamps the Rust state
//! in the git directory, and clippy does not run again until that state changes. When the agent already retried once
//! (`stop_hook_active`), let go to avoid an endless loop. Spec: `specs/clippy-gate.md`.
//!
//! Declare: Stop
//!
//! ```cargo
//! [dependencies]
//! serde_json = "1"
//! ```

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::Path;
use std::process::Command;

const MAX_DIAGNOSTICS: usize = 40;
/// The first word of the lines cargo prints to show its progress.
const PROGRESS: [&str; 10] = [
    "Compiling",
    "Checking",
    "Updating",
    "Downloading",
    "Downloaded",
    "Locking",
    "Adding",
    "Blocking",
    "Fresh",
    "Finished",
];

fn git(dir: &str, args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// The changed and untracked `.rs` files, outside the `.claude` directories whose scripts are not crate code.
fn changed_rust_files(dir: &str) -> Vec<String> {
    let status = git(dir, &["status", "--porcelain", "-z", "--untracked-files=all"]);
    let mut entries = status.split('\0').filter(|e| !e.is_empty());
    let mut files = Vec::new();
    while let Some(entry) = entries.next() {
        let (code, path) = entry.split_at(entry.len().min(3));
        let mut paths = vec![path];
        if code.starts_with('R') || code.starts_with('C') {
            // a rename or copy is followed by its source path
            paths.extend(entries.next());
        }
        files.extend(
            paths.into_iter().filter(|p| p.ends_with(".rs") && !p.split('/').any(|c| c == ".claude")).map(String::from),
        );
    }
    files
}

/// A hash of what clippy sees of the Rust code: the commit, and the path and content of each changed `.rs` file.
fn rust_state(dir: &str, files: &[String]) -> String {
    let mut hasher = DefaultHasher::new();
    git(dir, &["rev-parse", "HEAD"]).hash(&mut hasher);
    for file in files {
        file.hash(&mut hasher);
        std::fs::read(Path::new(dir).join(file)).unwrap_or_default().hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// A diagnostic line of `--message-format=short`: `path:line:column: level: message`.
fn is_diagnostic(line: &str) -> bool {
    let mut parts = line.splitn(4, ':');
    let (path, line_no, column) = (parts.next(), parts.next(), parts.next());
    let number = |p: Option<&str>| p.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    path.is_some_and(|p| !p.trim().is_empty()) && number(line_no) && number(column) && parts.next().is_some()
}

fn is_progress(line: &str) -> bool {
    let first = line.split_whitespace().next().unwrap_or("");
    PROGRESS.contains(&first) || line.contains("could not compile")
}

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok();
    let event: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    if event["stop_hook_active"].as_bool().unwrap_or(false) {
        return;
    }
    let dir = std::env::var("CLAUDE_PROJECT_DIR").unwrap_or_else(|_| ".".into());

    if !Path::new(&dir).join("Cargo.toml").exists() {
        return;
    }
    let files = changed_rust_files(&dir);
    if files.is_empty() {
        return;
    }
    let stamp = git(&dir, &["rev-parse", "--path-format=absolute", "--git-path", "claude-clippy-pass"]);
    let stamp = stamp.trim();
    let state = rust_state(&dir, &files);
    if !stamp.is_empty() && std::fs::read_to_string(stamp).is_ok_and(|s| s == state) {
        return;
    }

    let clippy = Command::new("cargo")
        .args(["clippy", "--workspace", "--all-targets", "--message-format=short", "--", "-D", "warnings"])
        .current_dir(&dir)
        .output()
        .expect("cargo must be installed");
    if clippy.status.success() {
        if !stamp.is_empty() {
            let _ = std::fs::write(stamp, state);
        }
        return;
    }

    let stderr = String::from_utf8_lossy(&clippy.stderr);
    // the located diagnostics and the errors with no location (a build script, a missing crate); the same
    // diagnostic comes once per target (lib, lib test): keep the first
    let mut seen = HashSet::new();
    let diagnostics: Vec<&str> = stderr
        .lines()
        .filter(|l| is_diagnostic(l) || (l.starts_with("error") && !is_progress(l)))
        .filter(|l| seen.insert(*l))
        .collect();
    eprintln!("cargo clippy reports problems; fix them (the project must stay warning-free):");
    if diagnostics.is_empty() {
        // nothing to point at: show the end of the output, without the build progress
        let lines: Vec<&str> = stderr.lines().filter(|l| !is_progress(l)).collect();
        eprintln!("{}", lines[lines.len().saturating_sub(MAX_DIAGNOSTICS)..].join("\n"));
    } else {
        eprintln!("{}", diagnostics.iter().take(MAX_DIAGNOSTICS).copied().collect::<Vec<_>>().join("\n"));
        if diagnostics.len() > MAX_DIAGNOSTICS {
            eprintln!("... {} more (run `cargo clippy` for the rest)", diagnostics.len() - MAX_DIAGNOSTICS);
        }
    }
    std::process::exit(2);
}
