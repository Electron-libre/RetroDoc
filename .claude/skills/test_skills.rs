#!/usr/bin/env rust-script
//! Structural check of the project skills: frontmatter is valid, `name` matches the directory,
//! and every skill (`skill:<name>`) or repo doc (`` `AGENTS.md` ``, `` `docs/….md` ``) a skill
//! references actually exists.
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

const OUTPUT_SECTION: &str = "## Reports to the user";
const REPORTING_SKILLS: [&str; 7] = [
    "issue-workflow",
    "create-issue",
    "prioritize-issues",
    "audit-instructions",
    "update-docs",
    "commit-message",
    "write-spec",
];
/// Wording that makes the agent narrate its steps or repeat the plan.
const NARRATION: [&str; 5] = ["report the result of each", "announce each", "announce the step", "narrate", "repeat the plan"];
/// Phrases of the output rule, as they stand in `AGENTS.md`; a skill citing the rule must not copy them.
const OUTPUT_RULES: [&str; 4] = [
    "table or a checklist",
    "no narration of the steps",
    "don't repeat what the diff",
    "only for a decision the user must take",
];

// spec: RPT-1
/// `AGENTS.md` holds the one section on the output of the agent, with its four rules.
fn check_output_section(root: &Path) -> Vec<String> {
    let text = fs::read_to_string(root.join("AGENTS.md")).unwrap_or_default();
    let Some(section) = text.split(OUTPUT_SECTION).nth(1) else {
        return vec![format!("AGENTS.md: missing '{OUTPUT_SECTION}' (output rule of the agent)")];
    };
    let section = section.split("\n## ").next().unwrap_or_default().to_lowercase();
    OUTPUT_RULES
        .iter()
        .filter(|rule| !section.contains(*rule))
        .map(|rule| format!("AGENTS.md: the output section does not state '{rule}'"))
        .collect()
}

// spec: RPT-2
/// Every reporting skill cites the section instead of restating its rules.
fn check_output_citation(name: &str, text: &str) -> Vec<String> {
    if !REPORTING_SKILLS.contains(&name) {
        return Vec::new();
    }
    let mut failures = Vec::new();
    if !text.contains("Reports to the user") {
        failures.push(format!("{name}: does not cite the 'Reports to the user' section of `AGENTS.md`"));
    }
    let lower = text.to_lowercase();
    for rule in OUTPUT_RULES.iter().filter(|rule| lower.contains(*rule)) {
        failures.push(format!("{name}: restates the output rule '{rule}' instead of citing it"));
    }
    failures
}

// spec: RPT-6
/// No skill asks the agent to narrate its steps or repeat the plan.
fn check_no_narration(name: &str, text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    NARRATION
        .iter()
        .filter(|phrase| lower.contains(*phrase))
        .map(|phrase| format!("{name}: asks for narration ('{phrase}')"))
        .collect()
}

// spec: RPT-3, RPT-7
/// The human review stop of `issue-workflow` keeps its layout and the markers the review hook relies on.
fn check_review_template(name: &str, text: &str) -> Vec<String> {
    if name != "issue-workflow" {
        return Vec::new();
    }
    let step = text
        .split("6. **Human review")
        .nth(1)
        .and_then(|s| s.split("\n7. ").next())
        .unwrap_or_default();
    if step.is_empty() {
        return vec![format!("{name}: step 4.6 (human review) not found")];
    }
    ["`done`", "`check | result`", "`Code review:`", "`no findings`", "`not verified`", "`decisions needed`"]
        .iter()
        .filter(|part| !step.contains(*part))
        .map(|part| format!("{name}: step 4.6 does not name {part}"))
        .collect()
}

// spec: RPT-4, RPT-5
/// The resume summary, the final summary, the delivery of `create-issue` and the reformulation name a layout.
fn check_report_layouts(name: &str, text: &str) -> Vec<String> {
    let required: &[&str] = match name {
        "issue-workflow" => &["`next` / `left` lines", "`goal`, `out of scope`, `questions`", "final summary: a table `deliverable | commit`"],
        "create-issue" => &["path, rank and one line"],
        _ => &[],
    };
    let mut failures: Vec<String> = required
        .iter()
        .filter(|part| !text.contains(*part))
        .map(|part| format!("{name}: does not name the layout '{part}'"))
        .collect();
    for old in ["summarize where it stands", "a two-line summary", "Restate the need in your own words"] {
        if text.contains(old) {
            failures.push(format!("{name}: still asks for a free report ('{old}')"));
        }
    }
    failures
}

fn main() {
    let root = fs::canonicalize(Path::new(file!()).parent().unwrap().join("../..")).unwrap();
    let skills = root.join(".claude/skills");
    let root_docs = config_list(&root, "root_docs");
    let mut failures = Vec::new();
    let mut visited = Vec::new();
    failures.extend(check_output_section(&root));

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
        failures.extend(check_output_citation(&name, &text));
        failures.extend(check_no_narration(&name, &text));
        failures.extend(check_review_template(&name, &text));
        failures.extend(check_report_layouts(&name, &text));
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
