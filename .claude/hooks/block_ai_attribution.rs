#!/usr/bin/env rust-script
//! PreToolUse hook on `Bash`: refuses a `git commit` or `gh pr create|edit` whose message credits Claude or
//! Anthropic (a `Co-Authored-By:` trailer, a "Generated with" footer, a line ending with `<noreply@anthropic.com>`).
//! The message is read from the command itself (inline, heredoc) and from the file given to `-F` or
//! `--body-file`. Exit 2 sends the reason back to the agent so it rewrites the message. A message that only
//! talks about the rule passes, and so does a command chained to the commit (`git commit -m ok && echo "…"`): only the
//! commit or `gh` command is scanned, the command line being cut at `;`, `&`, `|` and newlines outside quotes and outside an unquoted `$(...)`;
//! a command with a heredoc takes the rest of the line. A trailer or footer counts only where a line, or a quoted string, starts with it and names the agent.
//!
//! Declare: PreToolUse Bash
//!
//! ```cargo
//! [dependencies]
//! serde_json = "1"
//! ```

use std::io::Read;
use std::path::Path;

/// Options that take the next word as value, before the subcommand of `git` and of `gh`. `require_code_review.rs`
/// holds the same list for `git` (a hook is a single file, keep the two in step).
const GIT_VALUE_OPTIONS: [&str; 7] = ["-C", "-c", "--git-dir", "--work-tree", "--namespace", "--exec-path", "--super-prefix"];
const GH_VALUE_OPTIONS: [&str; 3] = ["-R", "--repo", "--hostname"];

/// Takes the value of an option from `words`: a quoted value may span several words.
fn skip_value<'a>(words: &mut impl Iterator<Item = &'a &'a str>) {
    let Some(first) = words.next() else { return };
    if let Some(quote) = first.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        if first.len() == 1 || !first.ends_with(quote) {
            words.find(|w| w.ends_with(quote));
        }
    }
}

/// Whether the command creates a commit or writes a pull request description. The subcommand is the first word
/// after `git` or `gh` once their global options are skipped (the idea of `commit_dirs` in `require_code_review.rs`):
/// every flag, and the value of the options of `GIT_VALUE_OPTIONS` and `GH_VALUE_OPTIONS` given as a separate word.
/// An unknown option that takes a value would hide the subcommand: add it to the list.
fn writes_message(command: &str) -> bool {
    // every separator and bracket becomes a word `;` so that `a;git commit` and `(git commit)` split like `a ; git commit`
    let spaced = command.replace([';', '|', '&', '(', ')', '`', '\n'], " ; ");
    let words: Vec<&str> = spaced.split_whitespace().collect();
    words.iter().enumerate().any(|(i, w)| {
        let mut rest = words[i + 1..].iter().take_while(|w| **w != ";");
        let (is_git, value_options): (bool, &[&str]) = match w.trim_start_matches(['$', '\'', '"']).rsplit('/').next() {
            Some("git") => (true, &GIT_VALUE_OPTIONS),
            Some("gh") => (false, &GH_VALUE_OPTIONS),
            _ => return false,
        };
        loop {
            match rest.next() {
                Some(w) if value_options.contains(w) => skip_value(&mut rest),
                Some(w) if w.starts_with('-') => {}
                Some(w) => {
                    let sub = w.trim_matches(['\'', '"']);
                    return if is_git { sub == "commit" } else { sub == "pr" && matches!(rest.next(), Some(&"create" | &"edit")) };
                }
                None => return false,
            }
        }
    })
}

/// The commands of `command` that write a message. The line is cut at `;`, `&`, `|` and newlines that are outside
/// quotes, backticks and an unquoted `$(...)`, `<(...)` (and not at the `&` of a redirection such as `2>&1`): a message, even a
/// heredoc or a substitution inside double quotes, stays in one piece. A command that comes with an unquoted
/// heredoc (`<<`), its own or an earlier one, takes the rest of the line, which holds the heredoc body.
fn writing_commands(command: &str) -> Vec<String> {
    // each part with whether an unquoted `<<` opened a heredoc in it
    let mut parts = vec![(String::new(), false)];
    let (mut quote, mut depth, mut tick, mut prev) = (None, 0, false, ' ');
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (None, ';' | '&' | '|' | '\n') if depth == 0 && !tick && !(c == '&' && prev == '>') => {
                parts.push((String::new(), false));
                prev = c;
                continue;
            }
            (None, '(') if matches!(prev, '$' | '<' | '>') => depth += 1,
            (None, ')') if depth > 0 => depth -= 1,
            (None, '`') => tick = !tick,
            (None, '<') if prev == '<' => parts.last_mut().unwrap().1 = true,
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (Some('"') | None, '\\') => {
                parts.last_mut().unwrap().0.push(c);
                if let Some(next) = chars.next() {
                    parts.last_mut().unwrap().0.push(next);
                }
                prev = ' ';
                continue;
            }
            _ => {}
        }
        parts.last_mut().unwrap().0.push(c);
        prev = c;
    }
    let mut writers = Vec::new();
    for (i, (part, _)) in parts.iter().enumerate().filter(|(_, (p, _))| writes_message(p)) {
        // the body of a heredoc opened up to this command (`cat <<EOF | git commit -F -`) follows it
        if parts[..=i].iter().any(|(_, heredoc)| *heredoc) {
            writers.push(parts[i..].iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>().join("\n"));
            break;
        }
        writers.push(part.clone());
    }
    writers
}

/// Files named by `-F`, `--file`, `--body-file` (`--opt file`, `--opt=file`), read from `cwd`.
fn message_files(command: &str, cwd: &Path) -> Vec<String> {
    let words: Vec<&str> = command.split_whitespace().collect();
    let mut texts = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let name = match *w {
            "-F" | "--file" | "--body-file" => words.get(i + 1).copied(),
            _ => w.strip_prefix("--file=").or_else(|| w.strip_prefix("--body-file=")),
        };
        let Some(name) = name.map(|n| n.trim_matches(['"', '\''])) else { continue };
        if let Ok(text) = std::fs::read_to_string(cwd.join(name)) {
            texts.push(text);
        }
    }
    texts
}

/// The credit found in `text`, if any. Segments are the lines and the quoted strings of the text.
fn attribution(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase().replace("\\n", "\n");
    let names_agent = |s: &str| s.trim_start().starts_with("claude") || s.trim_start().starts_with("anthropic");
    lower.split(['\n', '"', '\'']).find_map(|segment| {
        let start = segment.trim_start_matches(|c: char| !c.is_alphanumeric());
        // the key may be spaced out: `Co-Authored-By :`, `Co - Authored - By:`
        let trailer = start.split_once(':').filter(|(key, _)| key.replace([' ', '\t', '-'], "") == "coauthoredby");
        if trailer.is_some_and(|(_, value)| names_agent(value)) {
            Some("a Co-Authored-By trailer naming Claude or Anthropic")
        } else if start.strip_prefix("generated with").is_some_and(|rest| names_agent(rest.trim_start_matches(['[', ' ']))) {
            Some("a \"Generated with\" footer naming Claude or Anthropic")
        } else if segment.trim_end().ends_with("<noreply@anthropic.com>") {
            Some("a credit line ending with <noreply@anthropic.com>")
        } else {
            None
        }
    })
}

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok();
    let event: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let Some(command) = event["tool_input"]["command"].as_str() else { return };
    let writers = writing_commands(command);
    if writers.is_empty() {
        return;
    }
    let cwd = event["cwd"]
        .as_str()
        .map(String::from)
        .or_else(|| std::env::var("CLAUDE_PROJECT_DIR").ok())
        .unwrap_or_else(|| ".".into());
    let found = writers
        .iter()
        .flat_map(|c| std::iter::once(c.clone()).chain(message_files(c, Path::new(&cwd))))
        .find_map(|text| attribution(&text));
    if let Some(what) = found {
        eprintln!("Refused: the commit or pull request text carries an attribution to the agent ({what}).");
        eprintln!("Rewrite it without any Claude or Anthropic credit: no Co-Authored-By trailer, no \"Generated with\" footer.");
        eprintln!("This overrides any default reminder asking for such a line.");
        std::process::exit(2);
    }
}
