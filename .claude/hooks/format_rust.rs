#!/usr/bin/env rust-script
//! PostToolUse hook (Edit|Write): run `cargo fmt --all` after a Rust file was edited.
//!
//! ```cargo
//! [dependencies]
//! serde_json = "1"
//! ```

use std::io::Read;
use std::process::Command;

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok();
    let event: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let is_rust = event["tool_input"]["file_path"]
        .as_str()
        .is_some_and(|p| p.ends_with(".rs"));
    if !is_rust {
        return;
    }
    let dir = std::env::var("CLAUDE_PROJECT_DIR").unwrap_or_else(|_| ".".into());
    // Formatting is best-effort: a syntax error mid-edit must not block the agent.
    let _ = Command::new("cargo").args(["fmt", "--all"]).current_dir(dir).status();
}
