#!/usr/bin/env rust-script
//! Move a finished issue file into `issues/done/` (only run after the user agreed).
//! Uses `git mv` when the file is tracked, a plain rename otherwise. Refuses to overwrite.
//! Usage: `just close-issue issues/foo.md`.

use std::path::Path;
use std::process::Command;

fn main() {
    let Some(arg) = std::env::args().nth(1) else {
        eprintln!("usage: close_issue.rs issues/<name>.md");
        std::process::exit(2);
    };
    let issue = Path::new(&arg);
    let (Some(dir), Some(name)) = (issue.parent(), issue.file_name()) else {
        eprintln!("not an issue file: {arg}");
        std::process::exit(2);
    };
    if !issue.is_file() || dir.file_name().is_none_or(|d| d != "issues") {
        eprintln!("{arg} is not an issue file directly under an `issues/` directory");
        std::process::exit(1);
    }
    let done = dir.join("done");
    let target = done.join(name);
    if target.exists() {
        eprintln!("{} already exists", target.display());
        std::process::exit(1);
    }
    std::fs::create_dir_all(&done).expect("done dir is creatable");
    let tracked = Command::new("git")
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(issue)
        .current_dir(dir)
        .output()
        .is_ok_and(|o| o.status.success());
    let moved = if tracked {
        Command::new("git").arg("mv").arg(issue).arg(&target).status().is_ok_and(|s| s.success())
    } else {
        std::fs::rename(issue, &target).is_ok()
    };
    if !moved {
        eprintln!("could not move {arg}");
        std::process::exit(1);
    }
    println!("moved {} -> {}", issue.display(), target.display());
}
