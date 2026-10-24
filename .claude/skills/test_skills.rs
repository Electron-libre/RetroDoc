#!/usr/bin/env rust-script
//! Structural check of the project skills: frontmatter is valid, `name` matches the directory,
//! and every skill (`skill:<name>`) or repo doc (`` `AGENTS.md` ``, `` `docs/….md` ``) a skill
//! references actually exists.
//! Usage: `just test-harness` (or `rust-script .claude/skills/test_skills.rs`).

use std::fs;
use std::path::Path;

const BUILT_IN_ROOT_DOCS: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// Strings of `key = ["a", "b"]` (on one or several lines) in `.claude/harness.toml`; empty when absent.
fn config_list(root: &std::path::Path, key: &str) -> Vec<String> {
    let text = std::fs::read_to_string(root.join(".claude/harness.toml")).unwrap_or_default();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let value = line.strip_prefix(key).map(str::trim_start).and_then(|rest| rest.strip_prefix('='));
        if let Some(value) = value {
            let rest = &text[offset + line.len() - value.len()..];
            let list = rest.split_once('[').and_then(|(_, r)| r.split_once(']')).map_or("", |(list, _)| list);
            return list.split('"').skip(1).step_by(2).map(str::to_string).collect();
        }
        offset += line.len();
    }
    Vec::new()
}

fn is_doc_path(s: &str, root_docs: &[String]) -> bool {
    let path_chars = s.chars().all(|c| c.is_ascii_alphanumeric() || "_./-".contains(c));
    path_chars && (BUILT_IN_ROOT_DOCS.contains(&s) || root_docs.iter().any(|d| d == s) || (s.starts_with("docs/") && s.ends_with(".md")))
}

fn main() {
    let root = fs::canonicalize(Path::new(file!()).parent().unwrap().join("../..")).unwrap();
    let skills = root.join(".claude/skills");
    let root_docs = config_list(&root, "root_docs");
    let mut failures = Vec::new();

    let mut dirs: Vec<_> = fs::read_dir(&skills)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .collect();
    dirs.sort_by_key(fs::DirEntry::file_name);

    for entry in dirs {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(text) = fs::read_to_string(entry.path().join("SKILL.md")) else {
            failures.push(format!("{name}: SKILL.md missing"));
            continue;
        };
        let mut lines = text.lines();
        if lines.next() != Some("---") {
            failures.push(format!("{name}: no frontmatter"));
        }
        let front: Vec<&str> = lines.by_ref().take_while(|l| *l != "---").collect();
        if !front.contains(&format!("name: {name}").as_str()) {
            failures.push(format!("{name}: frontmatter name != directory"));
        }
        let long_description = front
            .iter()
            .any(|l| l.strip_prefix("description: ").is_some_and(|d| d.len() >= 40));
        if !long_description {
            failures.push(format!("{name}: description missing or too short"));
        }

        // `skill:<name>` references
        for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || ":-".contains(c))) {
            if let Some(target) = word.strip_prefix("skill:").filter(|t| !t.is_empty()) {
                if !skills.join(target).join("SKILL.md").exists() {
                    failures.push(format!("{name}: references unknown skill '{target}'"));
                }
            }
        }
        // `path` references to repo docs (odd segments between backticks)
        for path in text.split('`').skip(1).step_by(2).filter(|s| is_doc_path(s, &root_docs)) {
            if !root.join(path).exists() {
                failures.push(format!("{name}: references missing file '{path}'"));
            }
        }
        println!("checked {name}");
    }

    for f in &failures {
        println!("FAIL - {f}");
    }
    std::process::exit(i32::from(!failures.is_empty()));
}
