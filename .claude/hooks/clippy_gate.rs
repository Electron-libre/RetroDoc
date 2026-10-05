#!/usr/bin/env rust-script
//! Stop hook: if Rust files changed, clippy must be warning-free before the agent can finish.
//! Exit 2 sends clippy's output back to the agent so it fixes the code. When the agent already
//! retried once (`stop_hook_active`), let go to avoid an endless loop.
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
    if event["stop_hook_active"].as_bool().unwrap_or(false) {
        return;
    }
    let dir = std::env::var("CLAUDE_PROJECT_DIR").unwrap_or_else(|_| ".".into());

    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&dir)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    if !status.lines().any(|l| l.ends_with(".rs")) {
        return;
    }

    let clippy = Command::new("cargo")
        .args(["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])
        .current_dir(&dir)
        .output()
        .expect("cargo must be installed");
    if clippy.status.success() {
        return;
    }
    eprintln!("cargo clippy reports problems; fix them (the project must stay warning-free):");
    eprintln!("{}", String::from_utf8_lossy(&clippy.stderr));
    std::process::exit(2);
}
