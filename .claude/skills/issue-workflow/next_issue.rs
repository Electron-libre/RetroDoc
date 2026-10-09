#!/usr/bin/env rust-script
//! Print the next issue to work on: the first line of `issues/queue.md`, with its title and the rest of
//! the queue. Reports (stderr) issues missing from the queue and queue lines without a file.
//! Usage: `just next-issue` (from the repo root, or pass the `issues` directory as argument).

use std::path::Path;

fn title(dir: &Path, stem: &str) -> String {
    std::fs::read_to_string(dir.join(format!("{stem}.md")))
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("# ").map(str::to_string)))
        .unwrap_or_else(|| "(no file)".to_string())
}

/// `- [stem](stem.md) - why` gives `(stem, why)`; any other line (title, prose) is not an entry.
fn entry(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim().strip_prefix("- [")?;
    let (stem, rest) = rest.split_once("](")?;
    let why = rest.split_once(')').map_or("", |(_, w)| w.trim_start_matches(" - "));
    Some((stem, why))
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "issues".to_string());
    let dir = Path::new(&dir);
    let Ok(queue) = std::fs::read_to_string(dir.join("queue.md")) else {
        eprintln!("no {}/queue.md", dir.display());
        std::process::exit(2);
    };
    let entries: Vec<(&str, &str)> = queue.lines().filter_map(entry).collect();

    let mut drift = false;
    for (stem, _) in &entries {
        if !dir.join(format!("{stem}.md")).is_file() {
            eprintln!("queue line without a file: {stem}");
            drift = true;
        }
    }
    let mut unlisted: Vec<String> = std::fs::read_dir(dir)
        .map(|d| d.flatten().filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned())).collect())
        .unwrap_or_default();
    unlisted.retain(|s| s != "queue" && dir.join(format!("{s}.md")).is_file() && !entries.iter().any(|(e, _)| e == s));
    unlisted.sort();
    for s in &unlisted {
        eprintln!("issue missing from the queue: {s}");
        drift = true;
    }

    match entries.first() {
        None => println!("The queue is empty."),
        Some((stem, why)) => {
            println!("Next: issues/{stem}.md\n  {}\n  why here: {why}", title(dir, stem));
            for (i, (s, _)) in entries.iter().enumerate().skip(1) {
                println!("{:>3}. {s}", i + 1);
            }
        }
    }
    std::process::exit(i32::from(drift));
}
