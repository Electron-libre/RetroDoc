#!/usr/bin/env rust-script
//! Refuses a `git commit` until the code review has run on the current diff. Two modes, one per event:
//! `record` (PostToolUse on `Skill`) stores a marker when the `code-review` skill ends; `check` (PreToolUse
//! on `Bash`) compares the diff at `git commit` with the marker and exits 2 when the review is missing.
//!
//! The marker, `claude-code-review.json` in the git directory of the repository (so it is never a file of the
//! work tree, whatever the ignore rules), holds the `HEAD`, a hash of the diff against `HEAD` (tracked
//! changes and untracked files, the state a `git add -A && git commit` commits) and the files it covers.
//! A commit moves `HEAD`, which retires the marker. `code-review` runs forked, so its `PostToolUse` fires when
//! the review is over, not when the skill loads. Changes made afterwards (the fixes of the review) pass as long
//! as they stay in files the review saw. A commit with nothing to commit, an amend of a clean tree
//! included, one that only touches `issues/`, and a machine without `git` are not gated. The user waives the
//! review of the next commit by creating `claude-review-waived` in the git directory; the hook stamps it with
//! `HEAD` when it lets a commit through and removes it, without using it, at the first check after `HEAD` moved
//! (a command that does not commit, such as `--dry-run`, leaves it usable; after a commit the user creates it
//! again), and refuses a `Bash` command that writes it (a redirection, `touch`, `tee`, `cp`, a shell…; a commit
//! message or a `grep` that only names it passes; a deterrent, not a barrier: a `Write` of the file or a name built
//! in pieces is not caught).
//!
//! The review judged is the one of the repository the commit lands in, named by `-C`, `--work-tree`, `--git-dir`
//! and by `GIT_DIR` / `GIT_WORK_TREE` written or exported in the same line (`specs/review-gate.md`). A value of
//! these two variables the text does not give (`$X`, `$(…)`) is refused. A variable exported by an earlier command
//! of the session is invisible to the hook.
//!
//! A third mode, `native`, is run by the git `pre-commit` hook that `harness sync` installs: git has already
//! resolved the repository (`GIT_DIR`, `GIT_WORK_TREE`, the current directory), so it judges that one whatever the
//! text of the command was, with the same marker, snapshot, exemptions and waiver as `check`. It reads no event.
//! A native hook does not run under `--no-verify` / `-n` or a moved `core.hooksPath`, so `check` refuses a commit
//! carrying either and any command that sets `core.hooksPath` (`--get` and the words of a message excepted).
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

/// Global options of `git` that take the next word as value; the same list as `GIT_VALUE_OPTIONS` of
/// `block_ai_attribution.rs` (a hook is a single file, keep the two in step). `--git-dir` and `--work-tree` are also
/// read apart, as `-C`: their value names the repository.
const GIT_VALUE_OPTIONS: [&str; 5] = ["--git-dir", "--work-tree", "--namespace", "--exec-path", "--super-prefix"];

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

/// The `GIT_DIR` and `GIT_WORK_TREE` the environment of a command gives `git`, read from the words before it: the
/// assignments of the command itself (alone, after `env`, before a wrapper such as `sudo`), and those exported earlier
/// in the line (`export`, `declare`, `typeset`; a plain assignment is exported by a later `export NAME`) until an
/// `unset`. `env -u NAME` hides the variable for its command.
fn environment_repository(words: &[&str]) -> (Option<PathBuf>, Option<PathBuf>) {
    const NAMES: [&str; 2] = ["GIT_DIR", "GIT_WORK_TREE"];
    let (mut prefix, mut shell, mut exported) = ([None::<PathBuf>, None], [None::<PathBuf>, None], [None::<PathBuf>, None]);
    let (mut masked, mut exporting, mut unsetting, mut env_unset) = ([false; 2], false, false, false);
    // fish `set`: (exports, erases) while its flags are read, until the name or the end of the command
    let mut fish = None::<(bool, bool)>;
    let mut words = words.iter();
    while let Some(w) = words.next() {
        let named = NAMES.iter().position(|n| w == n);
        if let Some((export, erase)) = fish.as_mut().filter(|_| w.starts_with('-')) {
            *export |= *w == "--export" || (!w.starts_with("--") && w.contains('x'));
            *erase |= *w == "--erase" || (!w.starts_with("--") && w.contains('e'));
            continue;
        }
        if let (Some((export, erase)), true) = (fish.take(), *w != ";") {
            if let Some(n) = named {
                if erase {
                    (prefix[n], shell[n], exported[n]) = (None, None, None);
                } else if export {
                    // a substitution leaves the line without a value (`;`): the check of the raw command refuses it
                    let value = words.as_slice().first().filter(|v| **v != ";").and_then(|_| take_value(&mut words));
                    exported[n] = value.map(|v| PathBuf::from(v.trim_matches(['"', '\''])));
                }
            }
            continue;
        }
        if let Some(n) = NAMES.iter().position(|n| w.strip_prefix(n).is_some_and(|v| v.starts_with('='))) {
            let value = Some(PathBuf::from(quoted(&w[NAMES[n].len() + 1..], &mut words).trim_matches(['"', '\''])));
            if exporting {
                exported[n] = value;
            } else {
                if exported[n].is_some() {
                    exported[n] = value.clone();
                }
                (prefix[n], shell[n], masked[n]) = (value.clone(), value, false);
            }
        } else if *w == "set" {
            fish = Some((false, false));
        } else if *w == ";" {
            (prefix, masked, exporting, unsetting, env_unset) = ([None, None], [false; 2], false, false, false);
        } else if matches!(*w, "export" | "declare" | "typeset") {
            exporting = true;
        } else if *w == "unset" {
            unsetting = true;
        } else if *w == "-u" {
            env_unset = true;
        } else if let Some(n) = named {
            if unsetting {
                (prefix[n], shell[n], exported[n]) = (None, None, None);
            } else if env_unset {
                masked[n] = true;
            } else if exporting && shell[n].is_some() {
                exported[n] = shell[n].clone();
            }
            env_unset = false;
        } else {
            env_unset = false;
        }
    }
    let [dir, tree] = [0, 1].map(|n| if masked[n] { None } else { prefix[n].take().or(exported[n].take()) });
    (dir, tree)
}

/// Whether the word that opens `value` is expanded by the shell into something the text does not give (`$X`, `${X}`,
/// `$(…)`, backticks, unless in single quotes or escaped). Quotes and escapes decide where the word ends.
fn value_expands(value: &str) -> bool {
    let (mut single, mut double, mut escaped) = (false, false, false);
    for c in value.chars() {
        match c {
            _ if escaped => escaped = false,
            '\\' if !single => escaped = true,
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '$' | '`' if !single => return true,
            c if !single && !double && (c.is_whitespace() || matches!(c, ';' | '|' | '&')) => break,
            _ => {}
        }
    }
    false
}

/// The variable of `environment_repository` the line assigns a value the text does not give (`$X`, `${X}`, `$(…)`,
/// backticks, unless in single quotes), `None` when every value is literal. It reads the raw command, where the
/// brackets are still there.
fn undeterminable_variable(command: &str) -> Option<&'static str> {
    ["GIT_DIR", "GIT_WORK_TREE"].into_iter().find(|name| {
        command.match_indices(&format!("{name}=")).any(|(at, assignment)| {
            // `MY_GIT_DIR=` is another variable
            if command[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_') {
                return false;
            }
            value_expands(&command[at + assignment.len()..])
        })
    })
}

/// The variable of `environment_repository` a fish `set -x NAME value` of the line gives a value the text does not give
/// (`$X`, `(cmd)`, backticks, unless in single quotes), `None` when every value is literal. It reads the raw command.
fn fish_unreadable_variable(command: &str) -> Option<&'static str> {
    command.match_indices("set").find_map(|(at, _)| {
        // `reset` or `--set` is not the builtin
        if command[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '-') {
            return None;
        }
        let mut rest = command[at + 3..].strip_prefix(char::is_whitespace)?;
        let mut export = false;
        loop {
            rest = rest.trim_start();
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let word = &rest[..end];
            if let Some(long) = word.strip_prefix("--") {
                export |= long == "export";
            } else if let Some(short) = word.strip_prefix('-') {
                export |= short.contains('x');
            } else {
                let name = ["GIT_DIR", "GIT_WORK_TREE"].into_iter().find(|n| *n == word)?;
                let value = rest[end..].trim_start();
                return (export && (value.starts_with('(') || value_expands(value))).then_some(name);
            }
            rest = &rest[end..];
        }
    })
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

/// Commands that create or fill a file, or run text as a program, hence able to create the waiver wherever it stands
/// among their words.
const WRITERS: [&str; 17] =
    ["touch", "tee", "dd", "truncate", "sh", "bash", "zsh", "fish", "dash", "eval", "source", "python", "python3", "perl", "ruby", "node", "curl"];
/// Commands that write their last word (`cp a waiver`); `cp waiver /tmp/x` only reads it.
const COPIERS: [&str; 5] = ["cp", "mv", "ln", "install", "rsync"];

/// The commands of a line split at the separators that stand outside quotes, each twice: as written, and with the inside
/// of quotes, comments and heredoc bodies blanked (same byte offsets), so the words left are the ones the shell runs.
/// A quoted commit message or a heredoc fed to `git` is text, not a command. Where the text cannot be trusted a command
/// is kept whole and unblanked, so what it holds is read as code: a quote left open, or a substitution (`$(…)`, backtick)
/// inside double quotes, which the shell runs.
fn unquoted_commands(command: &str) -> Vec<(String, String)> {
    let command = command.replace("\\\n", " ");
    let blank = |s: &str| "_".repeat(s.len());
    let (mut commands, mut current, mut masked) = (Vec::new(), String::new(), String::new());
    let (mut quote, mut tainted, mut tags) = (None::<char>, false, Vec::<String>::new());
    let mut flush = |current: &mut String, masked: &mut String, tainted: &mut bool| {
        let (raw, blanked) = (std::mem::take(current), std::mem::take(masked));
        commands.push((raw.clone(), if std::mem::take(tainted) { raw } else { blanked }));
    };
    for line in command.split('\n') {
        if let Some(tag) = tags.first() {
            // inside a heredoc body: text for `git` or `cat`, but the words of a program for a shell
            if line.trim() == tag {
                tags.remove(0);
                if tags.is_empty() {
                    flush(&mut current, &mut masked, &mut tainted);
                }
            } else {
                current.push_str(line);
                masked.push_str(&blank(line));
            }
            current.push('\n');
            masked.push('\n');
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let mut put = |c: char, hide: bool, current: &mut String, masked: &mut String| {
                current.push(c);
                masked.push_str(&if hide { blank(&c.to_string()) } else { c.to_string() });
            };
            match quote {
                Some('\'') => {
                    quote = if c == '\'' { None } else { quote };
                    put(c, c != '\'', &mut current, &mut masked);
                }
                Some(_) => {
                    if c == '\\' {
                        put(c, true, &mut current, &mut masked);
                        i += 1;
                        if let Some(&n) = chars.get(i) {
                            put(n, true, &mut current, &mut masked);
                        }
                    } else {
                        tainted |= c == '`' || (c == '$' && chars.get(i + 1) == Some(&'('));
                        quote = if c == '"' { None } else { quote };
                        put(c, c != '"', &mut current, &mut masked);
                    }
                }
                None => match c {
                    '\\' => {
                        put(c, true, &mut current, &mut masked);
                        i += 1;
                        if let Some(&n) = chars.get(i) {
                            put(n, true, &mut current, &mut masked);
                        }
                    }
                    '\'' | '"' => {
                        quote = Some(c);
                        put(c, false, &mut current, &mut masked);
                    }
                    '#' if i == 0 || chars[i - 1].is_whitespace() => {
                        for &r in &chars[i..] {
                            put(r, true, &mut current, &mut masked);
                        }
                        break;
                    }
                    ';' | '|' | '&' | '(' | ')' | '`' => flush(&mut current, &mut masked, &mut tainted),
                    '<' if chars.get(i + 1) == Some(&'<') && chars.get(i + 2) != Some(&'<') => {
                        // `<<TAG`, `<<-TAG`, `<<'TAG'`: the lines up to `TAG` are its body
                        let rest: String = chars[i + 2..].iter().collect();
                        let tag: String = rest.trim_start_matches(['-', ' ']).chars().take_while(|c| !c.is_whitespace() && !matches!(c, ';' | '|' | '&' | ')')).collect();
                        let tag = tag.trim_matches(['\'', '"']).to_string();
                        if tag.chars().next().is_some_and(|f| f.is_alphabetic() || f == '_') {
                            tags.push(tag);
                        }
                        put(c, false, &mut current, &mut masked);
                    }
                    _ => put(c, false, &mut current, &mut masked),
                },
            }
            i += 1;
        }
        if quote.is_some() || !tags.is_empty() {
            current.push('\n');
            masked.push('\n');
        } else {
            flush(&mut current, &mut masked, &mut tainted);
        }
    }
    if quote.is_some() {
        // a quote left open: nothing can be told from the text
        return vec![(command.clone(), command)];
    }
    flush(&mut current, &mut masked, &mut tainted);
    commands.into_iter().filter(|(s, _)| !s.trim().is_empty()).collect()
}

/// Whether some command of the line writes the waiver: the name stands in it and either a redirection outside quotes
/// leads to it, or a command word (not a quoted one, so after `sudo`, `env`, `{`, `then`, `find -exec` as well) creates
/// files or runs text as a program (`WRITERS`), writes its last word (`COPIERS`) or edits in place (`sed -i`). A commit
/// message, a `grep` or a `cat` that only names the file is not one. A deterrent, not a barrier.
fn writes_waiver(command: &str) -> bool {
    unquoted_commands(command).iter().any(|(segment, masked)| {
        if !segment.contains(WAIVER) {
            return false;
        }
        let redirected = masked.match_indices('>').any(|(i, _)| {
            segment[i + 1..].trim_start_matches([' ', '>', '&', '|']).split_whitespace().next().is_some_and(|target| target.contains(WAIVER))
        });
        let last = segment.split_whitespace().last().is_some_and(|w| w.contains(WAIVER));
        let words: Vec<&str> = masked.split(|c: char| c.is_whitespace() || "\"'$(){};`<>".contains(c)).filter(|w| !w.is_empty()).collect();
        let writer = words.iter().any(|w| {
            let base = w.rsplit('/').next().unwrap_or(w);
            WRITERS.contains(&base) || (COPIERS.contains(&base) && last) || (base == "sed" && words.iter().any(|a| a.starts_with("-i") || *a == "--in-place"))
        });
        redirected || writer
    })
}

/// Whether some commit of the line skips its hooks: `commit_dirs` stops at the first one, so each command is read apart.
fn skips_hooks(command: &str) -> bool {
    segments(command).any(|segment| matches!(commit_dirs(segment), Some(Ok((_, true)))))
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

/// The line with a word `;` for each separator and brackets as words of their own, so that `a;git commit` and
/// `(git commit)` split like `a ; git commit`. A `&&`, `||`, `;` or newline is a plain `;`; a lone `|`, `&` and a
/// backtick are `;|`, `;&` and ``;` ``, which tell `shell_directory` that the command around them runs apart.
fn spaced_words(command: &str) -> String {
    let chars: Vec<char> = command.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '|' | '&' if chars.get(i + 1) == Some(&c) => {
                out.push_str(" ; ");
                i += 1;
            }
            // `2>&1`, `&>file`: a redirection
            '&' if chars.get(i + 1) == Some(&'>') || (i > 0 && chars[i - 1] == '>') => out.push_str(" ; "),
            '|' => out.push_str(" ;| "),
            '&' => out.push_str(" ;& "),
            '`' => out.push_str(" ;` "),
            ';' | '\n' => out.push_str(" ; "),
            '(' => out.push_str(" ; ( "),
            ')' => out.push_str(" ; ) "),
            _ => out.push(c),
        }
        i += 1;
    }
    out
}

/// The directory the `cd` among `words` (the ones before a `git`, the separators of `spaced_words`) lead the shell to,
/// `None` when there is none, and `"cd"` when a target is not given by the text (`$X`, `$(…)`, `~`, `-`, none, `popd`,
/// `pushd +1`). Relative targets accumulate. What runs apart does not move the shell: a subshell `( … )`, a backtick
/// substitution, a background command, a command of a pipeline; the effect of the first three is undone at their end.
fn shell_directory(words: &[&str]) -> (Option<PathBuf>, Option<&'static str>) {
    let (mut dir, mut unreadable) = (None::<PathBuf>, None::<&'static str>);
    let (mut saved, mut backtick, mut piped, mut at_start, mut i) = (Vec::new(), false, false, true, 0);
    let mut command_start = dir.clone();
    while i < words.len() {
        let w = words[i];
        i += 1;
        match w {
            ";" => {
                (command_start, piped, at_start) = (dir.clone(), false, true);
            }
            ";|" => (dir, piped, at_start) = (command_start.clone(), true, true),
            ";&" => (dir, at_start) = (command_start.clone(), true),
            "{" | "then" | "do" | "else" | "elif" | "if" | "while" | "until" | "!" | "time" | "builtin" | "command" | "and" | "or" | "begin" | "not" => {}
            "(" | ";`" if w == "(" || !backtick => {
                saved.push((dir.clone(), unreadable));
                backtick |= w == ";`";
                at_start = true;
            }
            ")" | ";`" => {
                if let Some((d, u)) = saved.pop() {
                    (dir, unreadable) = (d, u);
                }
                backtick &= w != ";`";
                at_start = false;
            }
            "cd" | "pushd" | "popd" if at_start && !piped => {
                at_start = false;
                while words.get(i).is_some_and(|a| *a == "--" || (a.len() > 1 && a.starts_with('-') && a[1..].chars().all(|c| "LPe@".contains(c)))) {
                    i += 1;
                }
                let end = words[i..].iter().position(|a| a.starts_with(';') || *a == ")" || *a == "(").map_or(words.len(), |p| i + p);
                let mut rest = words[i..end].iter();
                let target = take_value(&mut rest).filter(|v| w != "popd" && !value_expands(v) && !v.starts_with(['~', '-', '+']));
                i = end - rest.len();
                match target {
                    Some(v) => {
                        let v = PathBuf::from(v.trim_matches(['"', '\'']));
                        dir = Some(dir.map_or(v.clone(), |d| d.join(&v)));
                    }
                    None => {
                        unreadable.get_or_insert("cd");
                    }
                }
            }
            _ => at_start = false,
        }
    }
    (dir, unreadable)
}

/// The directories whose review a `git commit` of the line depends on, `None` when no command of the line commits;
/// `None` in the list is the shell directory. Normally one: after `-C dir`, `--work-tree dir` or `--git-dir dir` (the
/// work tree wins, then the parent of the innermost `.git` of the path, else the directory itself). With `--git-dir` and
/// no `--work-tree` there are two, since `git` commits the files of the directory it runs in into that git directory.
/// The `-C` options accumulate and a relative `--work-tree` or `--git-dir` is taken from the directory they lead to,
/// wherever they stand on the line, as `git` does. `GIT_DIR=x` and `GIT_WORK_TREE=x` assigned earlier in the same
/// command (alone, after `env`, before a wrapper such as `sudo`), or exported earlier in the line and not unset, count
/// as the two options, which win over them. The subcommand is the first word after `git` once the global options
/// (`SKIPPED_OPTIONS`, `GIT_VALUE_OPTIONS`, `-C dir`, `-c key=value`) are skipped; any other option means it is not a
/// commit. Brackets, separators and quotes around `git` are ignored; the `cd` before it set the directory the line starts from (`shell_directory`).
/// The `bool` of an `Ok` tells whether the commit skips the hooks (`skips_verification`).
/// A commit whose `-C`, `--git-dir` or `--work-tree` has a value the text does not give (`value_expands`, or none left
/// once a substitution split the line) is `Err` with the option: its repository is unknown.
fn commit_dirs(command: &str) -> Option<Result<(Vec<Option<String>>, bool), &'static str>> {
    // every separator and bracket becomes a word `;` so that `a;git commit` and `(git commit)` split like `a ; git commit`
    let spaced = spaced_words(&command.replace("\\\n", " "));
    // `shell_directory` tells the separators apart; everything else sees a plain `;`
    let kinded: Vec<&str> = spaced.split_whitespace().collect();
    let words: Vec<&str> = kinded.iter().map(|w| if w.starts_with(';') { ";" } else { w }).collect();
    words.iter().enumerate().find_map(|(i, w)| {
        if w.trim_start_matches(['$', '\'', '"']).rsplit('/').next() != Some("git") {
            return None;
        }
        let mut rest = words[i + 1..].iter().take_while(|w| **w != ";");
        let path = |v: &str| PathBuf::from(v.trim_matches(['"', '\'']));
        // the options set below win over the environment
        let (mut git_dir, mut work_tree) = environment_repository(&words[..i]);
        // the `cd` before `git` lead to the directory the relative paths of `-C` and of the repository options start from
        let (mut dir, mut unreadable) = shell_directory(&kinded[..i]);
        // the value of a repository option, `None` (and the option remembered) when the text does not give it
        let mut readable = |option: &'static str, value: Option<String>| {
            let value = value.filter(|v| !value_expands(v));
            if value.is_none() {
                unreadable.get_or_insert(option);
            }
            value
        };
        loop {
            match rest.next() {
                Some(&"-C") => {
                    if let Some(v) = readable("-C", take_value(&mut rest)) {
                        dir = Some(dir.map_or(path(&v), |d| d.join(path(&v))));
                    }
                }
                // `--git-dir` and `--work-tree` stay in `GIT_VALUE_OPTIONS` (same list as the other hook) but are read here first
                Some(&"--work-tree") => work_tree = readable("--work-tree", take_value(&mut rest)).map(|v| path(&v)),
                Some(&"--git-dir") => git_dir = readable("--git-dir", take_value(&mut rest)).map(|v| path(&v)),
                Some(w) if w.starts_with("--work-tree=") => {
                    work_tree = readable("--work-tree", Some(quoted(&w["--work-tree=".len()..], &mut rest))).map(|v| path(&v));
                }
                Some(w) if w.starts_with("--git-dir=") => {
                    git_dir = readable("--git-dir", Some(quoted(&w["--git-dir=".len()..], &mut rest))).map(|v| path(&v));
                }
                Some(w) if *w == "-c" || GIT_VALUE_OPTIONS.contains(w) => {
                    take_value(&mut rest);
                }
                Some(w) if SKIPPED_OPTIONS.contains(w) || GIT_VALUE_OPTIONS.iter().any(|o| w.strip_prefix(o).is_some_and(|v| v.starts_with('='))) => {}
                other => {
                    // a substitution splits the line, so the subcommand may stand after the `;` it left: fail closed
                    let is_commit = |w: &str| w.trim_end_matches(['\'', '"']) == "commit";
                    if let (None, Some(option)) = (other, unreadable) {
                        if words[i + 1..].iter().any(|w| is_commit(w)) {
                            return Some(Err(option));
                        }
                    }
                    if !other.is_some_and(|w| is_commit(w)) {
                        return None;
                    }
                    if let Some(option) = unreadable {
                        return Some(Err(option));
                    }
                    let skipped = skips_verification(&rest.copied().collect::<Vec<_>>());
                    // the directory `-C` leads to is where the relative paths are taken from
                    let from_dir = |p: PathBuf| dir.as_ref().map_or(p.clone(), |d| d.join(&p));
                    let text = |p: PathBuf| Some(p.to_string_lossy().into_owned());
                    if let Some(w) = work_tree {
                        return Some(Ok((vec![text(from_dir(w))], skipped)));
                    }
                    let Some(g) = git_dir else { return Some(Ok((vec![dir.and_then(text)], skipped))) };
                    let g = from_dir(g);
                    // the repository holding the git directory: the parent of its innermost `.git`, else itself
                    let repo = g.ancestors().find(|a| a.file_name().is_some_and(|n| n == ".git")).and_then(Path::parent).map_or(g.clone(), Path::to_path_buf);
                    let repo = if repo.as_os_str().is_empty() { PathBuf::from(".") } else { repo };
                    return Some(Ok((vec![text(repo), dir.and_then(text)], skipped)));
                }
            }
        }
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

/// Why the commit must wait for a review, or `None` when the review covers it.
fn refusal(cwd: &Path) -> Option<&'static str> {
    let snap = snapshot(cwd);
    let path = marker_path(cwd)?;
    // the waiver is spent by a commit, not by a command that may not commit (`--dry-run`, a refusal of another hook
    // or of git): it is stamped with `HEAD` when it lets a commit through and dropped once `HEAD` moved
    let stamp = if snap.head.is_empty() { NO_HEAD } else { snap.head.as_str() };
    let waiver = git_file(cwd, WAIVER).filter(|w| w.exists());
    let used = waiver.as_ref().map(|w| std::fs::read_to_string(w).unwrap_or_default().trim().to_string());
    if let (Some(waiver), Some(used)) = (&waiver, &used) {
        if !used.is_empty() && used != stamp {
            std::fs::remove_file(waiver).ok();
        }
    }
    if snap.files.is_empty() || snap.files.iter().all(|f| f.starts_with(ISSUES_DIR)) {
        return None;
    }
    if let (Some(waiver), Some(used)) = (&waiver, &used) {
        if used == stamp {
            return None;
        }
        if used.is_empty() {
            // fail closed: a waiver that cannot be stamped could never be spent
            if std::fs::write(waiver, stamp).is_err() {
                return Some("the waiver could not be stamped");
            }
            return None;
        }
    }
    let Some(marker) = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else {
        return Some("no code review has run in this project");
    };
    if marker["head"].as_str() != Some(snap.head.as_str()) {
        return Some("the last code review predates the last commit");
    }
    if marker["hash"].as_str() == Some(snap.hash.as_str()) {
        return None;
    }
    // a fix made after the review touches files the review already saw; a file it never saw is refused
    let reviewed: Vec<&str> = marker["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).collect();
    if snap.files.iter().all(|f| reviewed.contains(&f.as_str())) {
        return None;
    }
    Some("the diff holds files the last code review never saw")
}

/// Tells the agent the commit waits for a review and exits 2 (the one message of `check` and `native`).
fn refuse(why: &str) -> ! {
    eprintln!("Refused: {why}.");
    eprintln!("Run /code-review on the current diff with the Skill tool (issue-workflow, step 4.5), fix what it finds, then commit.");
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
        if let Some(why) = refusal(&toplevel(&here)) {
            refuse(why);
        }
        return;
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok();
    let event: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let project = std::env::var("CLAUDE_PROJECT_DIR").ok().map(PathBuf::from);
    // the top of the work tree costs a `git` call: asked only once the event is known to matter
    let shell_dir = event["cwd"].as_str().map(PathBuf::from).or(project).unwrap_or_else(|| ".".into());
    match mode.as_str() {
        "record" => {
            let skill = event["tool_input"]["skill"].as_str().unwrap_or_default();
            let succeeded = event["tool_response"]["success"].as_bool() == Some(true);
            if skill.rsplit(':').next() == Some(REVIEW_SKILL) && succeeded {
                if let Err(e) = record(&toplevel(&shell_dir)) {
                    eprintln!("Could not record the code review marker: {e}");
                    std::process::exit(1);
                }
            }
        }
        "check" => {
            let Some(command) = event["tool_input"]["command"].as_str() else { return };
            if writes_waiver(command) {
                eprintln!("Refused: only the user waives the code review, by creating {WAIVER} in the git directory.");
                if let Some(path) = git_file(&shell_dir, WAIVER) {
                    eprintln!("The user creates {} (a command that only names the file is not refused).", path.display());
                }
                std::process::exit(2);
            }
            if sets_hooks_path(command) {
                refuse_skipped_hook("setting core.hooksPath");
            }
            let Some(dirs) = commit_dirs(command) else { return };
            if skips_hooks(command) {
                refuse_skipped_hook("--no-verify or -n");
            }
            let unreadable = dirs.as_ref().err().map(|option| format!("the value of {option}")).or_else(|| undeterminable_variable(command).or_else(|| fish_unreadable_variable(command)).map(str::to_string));
            if let Some(what) = unreadable {
                eprintln!("Refused: {what} is one the hook cannot read (a variable, a substitution), so the repository the commit lands in is unknown.");
                eprintln!("Write the path in the command (`git -C dir commit`), or ask the user to waive the review.");
                std::process::exit(2);
            }
            let Ok((dirs, _)) = dirs else { return };
            let mut dirs = dirs;
            dirs.dedup();
            let mut roots: Vec<PathBuf> = dirs.into_iter().map(|d| toplevel(&d.map_or(shell_dir.clone(), |d| shell_dir.join(d)))).collect();
            roots.dedup();
            for cwd in roots {
                if let Some(why) = refusal(&cwd) {
                    refuse(why);
                }
            }
        }
        _ => {}
    }
}
