#!/usr/bin/env rust-script
//! Structural check of the project skills: frontmatter is valid, `name` matches the directory,
//! and every skill (`skill:<name>`), repo doc (`` `AGENTS.md` ``, `` `docs/….md` ``) or section of `AGENTS.md`
//! (`` `AGENTS.md` section "<heading>" ``) a skill references actually exists, as does every file of its own
//! directory it links (`[x](details.md)`); every markdown file of a skill is checked, not only `SKILL.md`.
//! Usage: `just test-harness` (or `rust-script .claude/skills/test_skills.rs`).
//! ```cargo
//! [dependencies]
//! toml = "0.8"
//! ```

use std::fs;
use std::path::Path;

const BUILT_IN_ROOT_DOCS: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// Passages a skill must keep, as (skill, passage, why).
const REQUIRED_PASSAGES: &[(&str, &str, &str)] = &[
    ("issue-workflow", "## Autonomous mode", "section defining the autonomous mode"),
    ("issue-workflow", "autonomous (default)", "plan validation asks autonomous or per-deliverable validation"),
    ("issue-workflow", "recorded first in `## Decisions`", "mode recorded in the issue so it survives a /clear"),
    ("issue-workflow", "Plan: proposed", "the plan is written in the issue before its validation"),
    ("issue-workflow", "set `Plan: validated`", "the validation is recorded so a resume skips the stop"),
    ("issue-workflow", "`1 a, 2 c`", "questions answered by number and option letter"),
];

/// Strings of `key = ["a", "b"]` in the `[project]` table of `.claude/harness.toml`; empty when absent.
fn config_list(root: &std::path::Path, key: &str) -> Vec<String> {
    let text = std::fs::read_to_string(root.join(".claude/harness.toml")).unwrap_or_default();
    let table: toml::Table = text.parse().unwrap_or_default();
    let list = table.get("project").and_then(|p| p.get(key)).or_else(|| table.get(key));
    list.and_then(toml::Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
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
    let mut visited = Vec::new();
    // headings of `AGENTS.md` outside fenced blocks; `None` when it cannot be read (its citations report that)
    let headings: Option<Vec<String>> = fs::read_to_string(root.join("AGENTS.md")).ok().map(|agents| {
        let mut fenced = false;
        agents
            .lines()
            .filter(|l| {
                fenced ^= l.starts_with("```");
                !fenced && l.starts_with('#')
            })
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .collect()
    });

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

        // the references below are checked in every markdown file of the skill, not only `SKILL.md`
        let mut extra: Vec<_> = fs::read_dir(entry.path())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "md") && !p.ends_with("SKILL.md"))
            .collect();
        extra.sort();
        let text = extra.iter().fold(text, |acc, p| acc + "\n" + &fs::read_to_string(p).unwrap_or_default());

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
        // relative markdown links outside code spans, resolved in the skill's directory
        let prose = text.split('`').step_by(2).collect::<Vec<_>>().join(" ");
        for link in prose.split("](").skip(1).filter_map(|t| t.split(')').next()) {
            // drop the title and the anchor; a target with a scheme (`https:`, `mailto:`) is not a file
            let target = link.split([' ', '#']).next().unwrap_or_default();
            if !target.is_empty() && !target.contains(':') && !entry.path().join(target).exists() {
                failures.push(format!("{name}: links the missing file '{target}'"));
            }
        }
        // `AGENTS.md` section "<heading>" references, wherever the text wraps
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for cited in flat.split("`AGENTS.md` section \"").skip(1) {
            let heading = cited.split('"').next().unwrap_or_default();
            if headings.as_ref().is_some_and(|h| !h.iter().any(|h| h == heading)) {
                failures.push(format!("{name}: cites the missing section '{heading}' of `AGENTS.md`"));
            }
        }
        for (skill, passage, why) in REQUIRED_PASSAGES.iter().filter(|(s, _, _)| *s == name) {
            if !text.contains(passage) {
                failures.push(format!("{skill}: missing '{passage}' ({why})"));
            }
        }
        visited.push(name.clone());
        println!("checked {name}");
    }

    for (skill, passage, _) in REQUIRED_PASSAGES.iter().filter(|(s, _, _)| !visited.iter().any(|v| v == s)) {
        failures.push(format!("{skill}: skill missing, cannot hold '{passage}'"));
    }

    for f in &failures {
        println!("FAIL - {f}");
    }
    std::process::exit(i32::from(!failures.is_empty()));
}
