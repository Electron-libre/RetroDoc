#!/usr/bin/env rust-script
//! Validate every non-merge commit of `<base>..<head>` with `just check-commit`.
//! Usage: `just check-commits <base> <head>` (used by the CI job on pull requests).

use std::process::{exit, Command};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [base, head] = args.as_slice() else {
        eprintln!("usage: check_commits.rs <base> <head>");
        exit(2);
    };
    let out = Command::new("git")
        .args(["rev-list", "--no-merges", &format!("{base}..{head}")])
        .output()
        .expect("cannot run git");
    if !out.status.success() {
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
        exit(1);
    }
    let mut failed = false;
    for rev in String::from_utf8_lossy(&out.stdout).lines() {
        println!("== {rev}");
        let ok = Command::new("just")
            .args(["check-commit", rev])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        failed |= !ok;
    }
    if failed {
        exit(1);
    }
}
