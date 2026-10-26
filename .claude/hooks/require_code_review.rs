#!/usr/bin/env rust-script
//! The code review gate of a commit. Three modes: `record` (PostToolUse on `Skill`) stores a marker when the
//! `code-review` skill ends; `native`, run by the git `pre-commit` hook that `harness sync` installs, compares the diff
//! with the marker and fails the commit when the review is missing; `check` (PreToolUse on `Bash`) refuses what would
//! skip that native hook (`specs/review-gate.md`, ADR 0005).
//!
//! The marker, `claude-code-review.json` in the git directory of the repository (so it is never a file of the
//! work tree, whatever the ignore rules), holds the `HEAD`, a hash of the diff against `HEAD` (tracked
//! changes and untracked files, the state an add of everything followed by a commit commits) and the files it covers.
//! A commit moves `HEAD`, which retires the marker. `code-review` runs forked, so its `PostToolUse` fires when
//! the review is over, not when the skill loads. Changes made afterwards (the fixes of the review) pass as long
//! as they stay in files the review saw. A commit with nothing to commit, an amend of a clean tree
//! included, one that only touches `issues/`, and a machine without `git` are not gated. The user waives the
//! review of the next commit by creating `claude-review-waived` in the git directory (`just waive-review`); the hook
//! stamps it with `HEAD` when it lets a commit through and removes it, without using it, at the first check after
//! `HEAD` moved (a command that does not commit, such as `--dry-run`, leaves it usable; after a commit the user
//! creates it again). The deny rules of the seeded `.claude/settings.json` keep the recipe and the file from the agent.
//!
//! `native` judges the repository git resolved (`GIT_DIR`, `GIT_WORK_TREE`, the current directory), whatever the text
//! of the command was, and reads no event. A native hook does not run under `--no-verify` / `-n` or a moved
//! `core.hooksPath`, so `check` refuses a commit carrying either and any command that sets `core.hooksPath` (`--get`
//! and the words of a message excepted). It reads the text only: the gate catches mistakes, not deliberate bypasses.
//!
//! Declare: PostToolUse Skill (argument `record`), PreToolUse Bash (argument `check`)
//!
//! ```cargo
//! [dependencies]
//! serde_json = "1"
//! ```

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const REVIEW_SKILL: &str = "code-review";
/// A commit of files under this directory only (the issues) needs no review.
const ISSUES_DIR: &str = "issues/";
/// Created by the user, in the git directory, to waive the review of the next commit; the hook removes it.
const WAIVER: &str = "claude-review-waived";
/// Stamp of a waiver used in a repository without commit.
const NO_HEAD: &str = "no-head";

/// What the working tree holds relative to `HEAD`.
#[derive(Debug, PartialEq)]
struct Snapshot {
    head: String,
    hash: String,
    files: Vec<String>,
}

fn git(cwd: &Path, args: &[&str], stdin: &[u8]) -> Vec<u8> {
    let Ok(mut child) = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };
    let mut pipe = child.stdin.take().unwrap();
    let input = stdin.to_vec();
    let writer = std::thread::spawn(move || pipe.write_all(&input).ok());
    let out = child.wait_with_output().map(|o| o.stdout).unwrap_or_default();
    writer.join().ok();
    out
}

/// Names from `-z` output (NUL separated, never quoted by git).
fn names(bytes: &[u8]) -> Vec<String> {
    bytes.split(|b| *b == 0).filter(|n| !n.is_empty()).map(|n| String::from_utf8_lossy(n).into_owned()).collect()
}

/// The object id `git` gives the content of each untracked file, which it reads itself (`--stdin-paths`: the content
/// of a large file never goes through this process). `-` stands for what is not a readable file (a directory such as a
/// nested repository, a dangling link); a name `--stdin-paths` cannot carry (it holds a line break) is hashed on its own.
fn untracked_hashes(cwd: &Path, names: &[String]) -> Vec<String> {
    let hash_one = |name: &str| {
        let out = git(cwd, &["hash-object", "--no-filters", "--", name], b"");
        String::from_utf8_lossy(&out).trim().to_string()
    };
    let hashable = |name: &String| cwd.join(name).metadata().is_ok_and(|m| m.is_file());
    let batch: Vec<&String> = names.iter().filter(|n| hashable(n) && !n.contains(['\n', '\r'])).collect();
    if batch.is_empty() {
        return names.iter().map(|_| "-".to_string()).collect();
    }
    let listing = batch.iter().map(|n| format!("{n}\n")).collect::<String>();
    let out = git(cwd, &["hash-object", "--no-filters", "--stdin-paths"], listing.as_bytes());
    let mut batched: Vec<String> = String::from_utf8_lossy(&out).lines().map(str::to_string).collect();
    // a file git could not read stops the batch: hash each one on its own
    if batched.len() != batch.len() {
        batched = batch.iter().map(|n| hash_one(n)).collect();
    }
    let mut batched = batch.into_iter().zip(batched).collect::<std::collections::HashMap<_, _>>();
    names
        .iter()
        .map(|n| batched.remove(n).or_else(|| hashable(n).then(|| hash_one(n))).filter(|h| !h.is_empty()).unwrap_or_else(|| "-".into()))
        .collect()
}

fn snapshot(cwd: &Path) -> Snapshot {
    let head = String::from_utf8_lossy(&git(cwd, &["rev-parse", "-q", "--verify", "HEAD"], b"")).trim().to_string();
    // the empty tree stands for HEAD in a repository without commit
    let base = if head.is_empty() { "4b825dc642cb6eb9a060e54bf8d69288fbee4904" } else { "HEAD" };
    let mut content = git(cwd, &["diff", base, "--binary", "--no-color", "--no-ext-diff", "--no-textconv"], b"");
    let mut files = names(&git(cwd, &["diff", base, "--name-only", "--no-renames", "-z"], b""));
    let untracked = names(&git(cwd, &["ls-files", "--others", "--exclude-standard", "-z"], b""));
    let hashes = untracked_hashes(cwd, &untracked);
    for (name, hash) in untracked.iter().zip(hashes) {
        content.extend(format!("\0{name}\0{hash}\0").into_bytes());
    }
    files.extend(untracked);
    files.sort();
    let hash = String::from_utf8_lossy(&git(cwd, &["hash-object", "--stdin"], &content)).trim().to_string();
    Snapshot { head, hash, files }
}

/// Global options of `git` that do not take the next word as value.
const SKIPPED_OPTIONS: [&str; 8] =
    ["--no-pager", "--paginate", "-p", "-P", "--no-optional-locks", "--bare", "--no-replace-objects", "--literal-pathspecs"];

/// Global options of `git` that take the next word as value.
const GIT_VALUE_OPTIONS: [&str; 7] = ["-C", "-c", "--git-dir", "--work-tree", "--namespace", "--exec-path", "--super-prefix"];

/// Completes `first`, the start of a value, with the next words of `words` when it opens a quote it does not close.
fn quoted<'a>(first: &str, words: &mut impl Iterator<Item = &'a &'a str>) -> String {
    let mut value = first.to_string();
    if let Some(quote) = first.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        if first.len() == 1 || !first.ends_with(quote) {
            for w in words.by_ref() {
                value = format!("{value} {w}");
                if w.ends_with(quote) {
                    break;
                }
            }
        }
    }
    value
}

/// Takes the value of an option from `words`: a quoted value may span several words.
fn take_value<'a>(words: &mut impl Iterator<Item = &'a &'a str>) -> Option<String> {
    let first = *words.next()?;
    Some(quoted(first, words))
}

/// Long options of the commit subcommand that take the next word as value.
const COMMIT_VALUE_OPTIONS: [&str; 12] =
    ["message", "file", "author", "date", "reuse-message", "reedit-message", "fixup", "squash", "template", "cleanup", "pathspec-from-file", "trailer"];

/// Whether the arguments of a commit skip its hooks: `--no-verify` (or an abbreviation git accepts) and `-n`, alone or
/// in a bundle (`-anm x`). The value of an option (`-m "fix -n"`, `--message x`) is not read; `--` ends the options.
fn skips_verification(args: &[&str]) -> bool {
    let mut args = args.iter();
    while let Some(w) = args.next() {
        if *w == "--" {
            return false;
        }
        if let Some(long) = w.strip_prefix("--") {
            if w.len() >= "--no-v".len() && "--no-verify".starts_with(w) {
                return true;
            }
            if COMMIT_VALUE_OPTIONS.contains(&long) {
                take_value(&mut args);
            }
        } else if let Some(shorts) = w.strip_prefix('-') {
            for (i, c) in shorts.char_indices() {
                if c == 'n' {
                    return true;
                }
                // the rest of a bundle after one of these is its value, which is the next word when nothing follows
                if "mFCctSu".contains(c) {
                    if i + 1 == shorts.len() && !"Su".contains(c) {
                        take_value(&mut args);
                    }
                    break;
                }
            }
        }
    }
    false
}

/// The commands of a line, split at the separators and brackets (a heredoc body or a quoted message is cut too, which
/// can only hide words from the readers below, never add some).
fn segments(command: &str) -> impl Iterator<Item = &str> {
    command.split([';', '|', '&', '(', ')', '`', '\n']).filter(|s| !s.trim().is_empty())
}

/// Whether a `git` command of `segment` (one command of a line) commits skipping its hooks (`skips_verification`).
/// The subcommand is the first word after `git` once its global options are skipped (`SKIPPED_OPTIONS`,
/// `GIT_VALUE_OPTIONS` as two words or as `--option=value`, a quoted value included); any other option means it is not
/// a commit.
fn commit_skips_hooks(segment: &str) -> bool {
    let words: Vec<&str> = segment.split_whitespace().collect();
    let is_git = |w: &&str| w.trim_start_matches(['$', '\'', '"']).rsplit('/').next() == Some("git");
    (0..words.len()).filter(|i| is_git(&words[*i])).any(|at| {
        let mut rest = words[at + 1..].iter();
        while let Some(w) = rest.next() {
            if GIT_VALUE_OPTIONS.contains(w) {
                take_value(&mut rest);
            } else if let Some(value) = GIT_VALUE_OPTIONS.iter().find_map(|o| w.strip_prefix(o)?.strip_prefix('=')) {
                quoted(value, &mut rest);
            } else if !SKIPPED_OPTIONS.contains(w) {
                return w.trim_end_matches(['\'', '"']) == "commit" && skips_verification(&rest.copied().collect::<Vec<_>>());
            }
        }
        false
    })
}

/// Whether some commit of the line skips its hooks, each command of the line read apart (a continued line joined).
fn skips_hooks(command: &str) -> bool {
    segments(&command.replace("\\\n", " ")).any(commit_skips_hooks)
}

/// Whether a command of the line sets `core.hooksPath` (`-c core.hooksPath=…`, `--config-env=core.hooksPath=…`,
/// `git config core.hooksPath value`, `GIT_CONFIG_KEY_n=core.hooksPath`), which moves the hooks away from the native
/// gate. Reading it (`--get`), unsetting it and the words of a commit message are not setting it; the exemptions hold
/// for the command they stand in only.
fn sets_hooks_path(command: &str) -> bool {
    segments(command).any(|segment| {
        let words: Vec<&str> = segment.split_whitespace().collect();
        let mentions_git = words.iter().any(|w| w.trim_start_matches(['$', '\'', '"']).rsplit('/').next() == Some("git") || w.starts_with("GIT_CONFIG"));
        if !mentions_git || words.iter().any(|w| w.starts_with("--get") || w.starts_with("--unset")) {
            return false;
        }
        let configuring = words.contains(&"config");
        let mut rest = words.iter();
        while let Some(w) = rest.next() {
            if *w == "--message" || (w.starts_with('-') && !w.starts_with("--") && w.ends_with('m')) {
                take_value(&mut rest);
                continue;
            }
            if let Some(message) = w.strip_prefix("--message=").or_else(|| w.strip_prefix("-m").filter(|m| !m.is_empty() && !w.starts_with("--"))) {
                quoted(message, &mut rest);
                continue;
            }
            let lower = w.to_lowercase();
            let assigned = lower.starts_with("core.hookspath=")
                || lower.starts_with("--config-env=core.hookspath")
                || (w.starts_with("GIT_CONFIG_KEY") && lower.ends_with("=core.hookspath"))
                || (w.starts_with("GIT_CONFIG_PARAMETERS") && lower.contains("core.hookspath"));
            // `git config core.hooksPath value` sets it, `git config core.hooksPath` reads it
            if assigned || (configuring && lower == "core.hookspath" && rest.len() > 0) {
                return true;
            }
        }
        false
    })
}

/// The root of the work tree `cwd` is in (git lists untracked files under the current directory only).
fn toplevel(cwd: &Path) -> PathBuf {
    let out = git(cwd, &["rev-parse", "--show-toplevel"], b"");
    let root = String::from_utf8_lossy(&out).trim().to_string();
    if root.is_empty() { cwd.to_path_buf() } else { PathBuf::from(root) }
}

/// A file of the git directory of the repository `cwd` belongs to, `None` outside a repository.
fn git_file(cwd: &Path, name: &str) -> Option<PathBuf> {
    let out = git(cwd, &["rev-parse", "--git-path", name], b"");
    let path = String::from_utf8_lossy(&out).trim().to_string();
    (!path.is_empty()).then(|| cwd.join(path))
}

fn marker_path(cwd: &Path) -> Option<PathBuf> {
    git_file(cwd, "claude-code-review.json")
}

fn record(cwd: &Path) -> std::io::Result<()> {
    let Some(path) = marker_path(cwd) else { return Ok(()) };
    let snap = snapshot(cwd);
    std::fs::write(path, serde_json::json!({"head": snap.head, "hash": snap.hash, "files": snap.files}).to_string())
}

/// Why the commit must wait for a review, and the waiver the hook could not use, if any.
struct Refusal {
    why: &'static str,
    /// A waiver that waives nothing because the hook could not use it: its path and what failed (`read`, `stamped`).
    broken_waiver: Option<(PathBuf, &'static str)>,
}

/// The waiver of the repository as the hook finds it.
enum Waiver {
    Absent,
    /// It exists but reading it failed: it is neither stamped nor dropped.
    Unreadable(PathBuf),
    /// Its content, trimmed: empty while unspent, then the `HEAD` it was stamped with.
    Text(PathBuf, String),
}

/// The waiver of `cwd`; one gone before the read is absent, content that is not UTF-8 is content, not a read failure.
fn waiver(cwd: &Path) -> Waiver {
    let Some(path) = git_file(cwd, WAIVER) else { return Waiver::Absent };
    match std::fs::read(&path) {
        Ok(bytes) => Waiver::Text(path, String::from_utf8_lossy(&bytes).trim().to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Waiver::Absent,
        Err(_) => Waiver::Unreadable(path),
    }
}

/// Why the commit must wait for a review, or `None` when the review covers it.
fn refusal(cwd: &Path) -> Option<Refusal> {
    let snap = snapshot(cwd);
    let path = marker_path(cwd)?;
    // the waiver is spent by a commit, not by a command that may not commit (`--dry-run`, a refusal of another hook
    // or of git): it is stamped with `HEAD` when it lets a commit through and dropped once `HEAD` moved
    let stamp = if snap.head.is_empty() { NO_HEAD } else { snap.head.as_str() };
    let mut waiver = waiver(cwd);
    if let Waiver::Text(path, used) = &waiver {
        if !used.is_empty() && used != stamp {
            std::fs::remove_file(path).ok();
            waiver = Waiver::Absent;
        }
    }
    if snap.files.is_empty() || snap.files.iter().all(|f| f.starts_with(ISSUES_DIR)) {
        return None;
    }
    // fail closed: a waiver that cannot be read or stamped could never be spent, so the review alone judges the commit
    let broken_waiver = match waiver {
        Waiver::Absent => None,
        Waiver::Unreadable(path) => Some((path, "read")),
        Waiver::Text(_, used) if used == stamp => return None,
        Waiver::Text(path, _) => {
            if std::fs::write(&path, stamp).is_ok() {
                return None;
            }
            Some((path, "stamped"))
        }
    }
    .map(|(path, failed)| (path.canonicalize().unwrap_or(path), failed));
    let refused = |why| Some(Refusal { why, broken_waiver: broken_waiver.clone() });
    let Some(marker) = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else {
        return refused("no code review has run in this project");
    };
    if marker["head"].as_str() != Some(snap.head.as_str()) {
        return refused("the last code review predates the last commit");
    }
    if marker["hash"].as_str() == Some(snap.hash.as_str()) {
        return None;
    }
    // a fix made after the review touches files the review already saw; a file it never saw is refused
    let reviewed: Vec<&str> = marker["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).collect();
    if snap.files.iter().all(|f| reviewed.contains(&f.as_str())) {
        return None;
    }
    refused("the diff holds files the last code review never saw")
}

/// Tells the agent the commit waits for a review, and to have the user fix a waiver the hook could not use, then exits 2.
fn refuse(refusal: Refusal) -> ! {
    eprintln!("Refused: {}.", refusal.why);
    eprintln!("Run /code-review low on the current diff with the Skill tool (issue-workflow, step 3.5), fix what it finds, then commit.");
    if let Some((waiver, failed)) = refusal.broken_waiver {
        eprintln!("The review waiver {} could not be {failed}, so it waives nothing: ask the user to fix that file.", waiver.display());
    }
    std::process::exit(2);
}

/// Refuses what would skip the native `pre-commit` hook, the last-resort gate that only this hook can defend.
fn refuse_skipped_hook(what: &str) -> ! {
    eprintln!("Refused: {what} skips the native pre-commit review hook that `harness sync` installs.");
    eprintln!("Commit without it, or ask the user to waive the review.");
    std::process::exit(2);
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode == "native" {
        // run by git as `pre-commit`: git resolved the repository (`GIT_DIR`, `GIT_WORK_TREE`, the current directory), and
        // there is no event on stdin to read
        let here = std::env::current_dir().unwrap_or_else(|_| ".".into());
        // `git commit -a` hands its hook a temporary index: the review judges the real one, as `check` does; a relative
        // `GIT_DIR` or `GIT_WORK_TREE` is only valid from this directory, the commands below run from the top of the tree
        std::env::remove_var("GIT_INDEX_FILE");
        for name in ["GIT_DIR", "GIT_WORK_TREE"] {
            if let Some(value) = std::env::var_os(name).filter(|v| Path::new(v).is_relative()) {
                std::env::set_var(name, here.join(value));
            }
        }
        if let Some(refusal) = refusal(&toplevel(&here)) {
            refuse(refusal);
        }
        return;
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok();
    let event: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    match mode.as_str() {
        "record" => {
            let skill = event["tool_input"]["skill"].as_str().unwrap_or_default();
            let succeeded = event["tool_response"]["success"].as_bool() == Some(true);
            if skill.rsplit(':').next() == Some(REVIEW_SKILL) && succeeded {
                let project = std::env::var("CLAUDE_PROJECT_DIR").ok().map(PathBuf::from);
                let shell_dir = event["cwd"].as_str().map(PathBuf::from).or(project).unwrap_or_else(|| ".".into());
                if let Err(e) = record(&toplevel(&shell_dir)) {
                    eprintln!("Could not record the code review marker: {e}");
                    std::process::exit(1);
                }
            }
        }
        "check" => {
            let Some(command) = event["tool_input"]["command"].as_str() else { return };
            if sets_hooks_path(command) {
                refuse_skipped_hook("setting core.hooksPath");
            }
            if skips_hooks(command) {
                refuse_skipped_hook("--no-verify or -n");
            }
        }
        _ => {}
    }
}
