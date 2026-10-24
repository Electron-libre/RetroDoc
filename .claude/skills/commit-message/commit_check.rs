#!/usr/bin/env rust-script
//! Validate a commit message read from stdin against the rules of `AGENTS.md`.
//! Prints each violation and exits 1; prints nothing and exits 0 when the message is valid.
//! Usage: `echo "$msg" | rust-script .claude/skills/commit-message/commit_check.rs`
//! or `just check-commit` (checks the message of HEAD).

use std::io::Read;

const TYPES: [&str; 9] = ["feat", "fix", "refactor", "docs", "test", "perf", "build", "ci", "chore"];
const BUILT_IN_SCOPES: [&str; 2] = ["docs", "harness"];
const ATTRIBUTION: [&str; 3] = ["co-authored-by", "generated with", "noreply@anthropic.com"];

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

/// The built-in scopes plus the project's `commit_scopes`.
fn scopes() -> Vec<String> {
    let mut all: Vec<String> = BUILT_IN_SCOPES.iter().map(|s| (*s).to_string()).collect();
    all.extend(config_list(std::path::Path::new("."), "commit_scopes"));
    all
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F000..=0x1FFFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF)
}

fn check_subject(subject: &str, errors: &mut Vec<String>) {
    if subject.chars().count() > 72 {
        errors.push(format!("subject is {} chars, the maximum is 72", subject.chars().count()));
    }
    let Some((head, description)) = subject.split_once(": ") else {
        errors.push("format must be `<type>(<scope>): <description>` (colon then one space)".into());
        return;
    };
    let (ty, scope) = match head.split_once('(') {
        Some((ty, rest)) => match rest.strip_suffix(')') {
            Some(scope) => (ty, Some(scope)),
            None => {
                errors.push("format: unclosed scope parenthesis".into());
                return;
            }
        },
        None => (head, None),
    };
    if !TYPES.contains(&ty) {
        errors.push(format!("unknown type `{ty}`, use one of {TYPES:?}"));
    }
    let scopes = scopes();
    if let Some(scope) = scope.filter(|s| !scopes.contains(&s.to_string())) {
        errors.push(format!("unknown scope `{scope}`, use one of {scopes:?} (or omit it)"));
    }
    if description.chars().next().is_some_and(char::is_uppercase) {
        errors.push("description must start in lowercase".into());
    }
    if description.ends_with('.') {
        errors.push("description must not end with a period".into());
    }
}

fn main() {
    let mut message = String::new();
    std::io::stdin().read_to_string(&mut message).expect("stdin is readable");
    let lines: Vec<&str> = message.lines().collect();
    let mut errors = Vec::new();

    check_subject(lines.first().copied().unwrap_or(""), &mut errors);
    if lines.get(1).is_some_and(|l| !l.is_empty()) {
        errors.push("a blank line must separate the subject from the body".into());
    }
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        if i > 0 && line.chars().count() > 72 {
            errors.push(format!("line {n} is {} chars, wrap at 72", line.chars().count()));
        }
        let lower = line.to_lowercase();
        if ATTRIBUTION.iter().any(|a| lower.contains(a)) {
            errors.push(format!("line {n}: no Co-Authored-By trailer or AI/tool attribution allowed"));
        }
        if i > 0 && (line.starts_with('#') || line.starts_with("```")) {
            errors.push(format!("line {n}: no markdown headings or code fences"));
        }
        if line.chars().any(is_emoji) {
            errors.push(format!("line {n}: no emoji"));
        }
    }

    for e in &errors {
        println!("{e}");
    }
    std::process::exit(i32::from(!errors.is_empty()));
}
