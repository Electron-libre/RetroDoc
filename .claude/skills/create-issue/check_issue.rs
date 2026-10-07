#!/usr/bin/env rust-script
//! Validate a new issue file against the template of `skill:create-issue`.
//! Usage: `just check-issue issues/foo.md`. Prints one line per problem, exit 1 if any.

use std::path::Path;

fn problems(path: &Path, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let stem = name.strip_suffix(".md").unwrap_or("");
    let snake = !stem.is_empty()
        && stem.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !snake || path.parent().and_then(Path::file_name).is_none_or(|d| d != "issues") {
        out.push("file must be `issues/<snake_case>.md`".to_string());
    }
    if path.parent().is_some_and(|d| d.join("done").join(name).exists()) {
        out.push(format!("`issues/done/{name}` already exists: pick another name"));
    }
    let headings: Vec<&str> = text.lines().filter(|l| l.starts_with("# ")).collect();
    match headings.first() {
        None => out.push("no `# <Title>` heading".to_string()),
        Some(t) if t.trim() == "# Goal" => out.push("first heading must be the title, not `# Goal`".to_string()),
        Some(_) => {}
    }
    for required in ["# Goal", "# Approach", "# Resources"] {
        if !headings.iter().any(|h| h.trim() == required) {
            out.push(format!("missing section `{required}`"));
        }
    }
    if headings.iter().any(|h| h.trim() == "# Tracking") {
        out.push("`# Tracking` is written by issue-workflow after the plan is validated".to_string());
    }
    // `# Goal` must have a body.
    let mut lines = text.lines();
    if lines.by_ref().any(|l| l.trim() == "# Goal")
        && lines.take_while(|l| !l.starts_with("# ")).all(|l| l.trim().is_empty())
    {
        out.push("`# Goal` is empty".to_string());
    }
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.contains("/home/") || line.contains("~/") || line.contains("/Users/") {
            out.push(format!("line {n}: private path"));
        }
        if line.to_ascii_lowercase().contains("co-authored-by") {
            out.push(format!("line {n}: attribution line"));
        }
    }
    out
}

fn main() {
    let Some(arg) = std::env::args().nth(1) else {
        eprintln!("usage: check_issue.rs issues/<name>.md");
        std::process::exit(2);
    };
    let path = Path::new(&arg);
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("cannot read {arg}");
        std::process::exit(2);
    };
    let found = problems(path, &text);
    for p in &found {
        println!("{arg}: {p}");
    }
    std::process::exit(i32::from(!found.is_empty()));
}
