#!/usr/bin/env rust-script
//! Validate a commit message read from stdin against the rules of `AGENTS.md`.
//! Prints each violation and exits 1; prints nothing and exits 0 when the message is valid.
//! Exits 2 with an `error:` line on stderr when `.claude/harness.toml` cannot be read from the repository root.
//! Usage: `echo "$msg" | rust-script .claude/skills/commit-message/commit_check.rs`
//! or `just check-commit` (checks the message of HEAD).
//! ```cargo
//! [dependencies]
//! toml = "0.8"
//! ```

use std::io::Read;

const TYPES: [&str; 9] = ["feat", "fix", "refactor", "docs", "test", "perf", "build", "ci", "chore"];
const BUILT_IN_SCOPES: [&str; 2] = ["docs", "harness"];
const ATTRIBUTION: [&str; 3] = ["co-authored-by", "generated with", "noreply@anthropic.com"];

/// Root of the repository holding the current directory.
fn repo_root() -> Result<std::path::PathBuf, String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("cannot run git to find the repository root: {e}"))?;
    if !out.status.success() {
        return Err("not inside a git repository: cannot locate .claude/harness.toml".into());
    }
    let root = String::from_utf8_lossy(&out.stdout);
    Ok(root.strip_suffix('\n').unwrap_or(&root).into())
}

/// Strings of `key = ["a", "b"]` in the `[project]` table of `.claude/harness.toml`; empty when the key is absent.
fn config_list(root: &std::path::Path, key: &str) -> Result<Vec<String>, String> {
    let path = root.join(".claude/harness.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let table: toml::Table = text.parse().map_err(|e| format!("invalid {}: {e}", path.display()))?;
    let list = table.get("project").and_then(|p| p.get(key)).or_else(|| table.get(key));
    Ok(list
        .and_then(toml::Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default())
}

/// The built-in scopes plus the project's `commit_scopes`, read once from the repository root.
fn scopes() -> Result<Vec<String>, String> {
    let mut all: Vec<String> = BUILT_IN_SCOPES.iter().map(|s| (*s).to_string()).collect();
    all.extend(config_list(&repo_root()?, "commit_scopes")?);
    Ok(all)
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F000..=0x1FFFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF)
}

fn check_subject(subject: &str, scopes: &[String], errors: &mut Vec<String>) {
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
    if let Some(scope) = scope.filter(|s| !scopes.iter().any(|k| k == s)) {
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
    let scopes = scopes().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(2);
    });

    check_subject(lines.first().copied().unwrap_or(""), &scopes, &mut errors);
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
