#!/usr/bin/env rust-script
//! Give an issue its own git worktree and branch, so that two agents can work on two issues at once.
//! `start issues/<stem>.md` creates the worktree `<main worktree>-<stem>` next to the main one, on a new branch
//! `<stem>` cut from the branch of the main worktree. `finish <stem> [--force]` removes that worktree and branch once the
//! branch is merged into the branch of the main worktree (ancestor, or every commit has a patch-equivalent there,
//! or a merge into it would change no file, which covers a rebase and a squash of any number of commits); `--force` also
//! accepts a branch whose work is not on it, after the user agreed, and still refuses a worktree with uncommitted
//! changes. `start` runs from any worktree; `finish` too, except from inside the worktree it would remove (it refuses and names the main worktree, `--force` or not).
//! Usage: `just start-issue issues/foo.md`, `just finish-issue foo` (`just finish-issue foo --force`).

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().map_err(|e| format!("cannot run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn git_ok(dir: &Path, args: &[&str]) -> bool {
    git(dir, args).is_ok()
}

/// The main worktree and the branch it has checked out (`None` when detached).
fn main_worktree(cwd: &Path) -> Result<(PathBuf, Option<String>), String> {
    let list = git(cwd, &["worktree", "list", "--porcelain"]).map_err(|_| "not inside a git repository".to_string())?;
    let record = list.split("\n\n").next().unwrap_or_default();
    if record.lines().any(|l| l == "bare") {
        return Err("a bare repository has no main worktree".to_string());
    }
    let path = record.lines().find_map(|l| l.strip_prefix("worktree ")).ok_or("no worktree found")?;
    let branch = record.lines().find_map(|l| l.strip_prefix("branch refs/heads/")).map(str::to_string);
    Ok((PathBuf::from(path), branch))
}

/// The path of the worktree that has `branch` checked out.
fn worktree_of(cwd: &Path, branch: &str) -> Option<PathBuf> {
    let list = git(cwd, &["worktree", "list", "--porcelain"]).ok()?;
    list.split("\n\n").find_map(|record| {
        let on = record.lines().any(|l| l.strip_prefix("branch refs/heads/") == Some(branch));
        on.then(|| record.lines().find_map(|l| l.strip_prefix("worktree ")).map(PathBuf::from)).flatten()
    })
}

fn sibling(main: &Path, stem: &str) -> Result<PathBuf, String> {
    let name = main.file_name().ok_or("the main worktree has no name")?.to_string_lossy();
    Ok(main.with_file_name(format!("{name}-{stem}")))
}

fn valid_stem(stem: &str) -> bool {
    !stem.is_empty() && !stem.starts_with('-') && stem != "queue" && stem.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn start(cwd: &Path, issue: &str) -> Result<String, String> {
    let path = Path::new(issue);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    if path.extension().is_none_or(|e| e != "md") || path.parent().and_then(Path::file_name).is_none_or(|d| d != "issues") || !valid_stem(stem) {
        return Err(format!("{issue} is not an issue file directly under an `issues/` directory"));
    }
    let (main, branch) = main_worktree(cwd)?;
    let base = branch.ok_or("the main worktree has no branch checked out")?;
    if git_ok(&main, &["rev-parse", "--verify", "-q", &format!("refs/heads/{stem}")]) {
        return Err(format!("the branch {stem} already exists: the issue is taken"));
    }
    if !git_ok(&main, &["cat-file", "-e", &format!("{base}:issues/{stem}.md")]) {
        return Err(format!("{issue} is not committed on {base}: commit it first, the worktree would not hold it"));
    }
    if !git(&main, &["status", "--porcelain", "--", &format!("issues/{stem}.md")])?.is_empty() {
        return Err(format!("{issue} has uncommitted changes in the main worktree: commit them first, the worktree would not hold them"));
    }
    let dir = sibling(&main, stem)?;
    if dir.exists() {
        return Err(format!("{} already exists", dir.display()));
    }
    git(&main, &["worktree", "add", "-q", "-b", stem, &dir.to_string_lossy(), &base])?;
    Ok(dir.display().to_string())
}

fn finish(cwd: &Path, stem: &str, force: bool) -> Result<String, String> {
    if !valid_stem(stem) {
        return Err(format!("not an issue stem: {stem}"));
    }
    let (main, branch) = main_worktree(cwd)?;
    let trunk = branch.ok_or("the main worktree has no branch checked out")?;
    if !git_ok(&main, &["rev-parse", "--verify", "-q", &format!("refs/heads/{stem}")]) {
        return Err(format!("no branch {stem}"));
    }
    let dir = sibling(&main, stem)?;
    // Removing the directory the caller stands in breaks its shell: refuse, whatever `--force` says.
    let inside = std::fs::canonicalize(cwd).ok().zip(std::fs::canonicalize(&dir).ok()).is_some_and(|(here, target)| here.starts_with(target));
    if inside {
        return Err(format!("the current directory is inside {}: run finish-issue from the main worktree {}; nothing was removed", dir.display(), main.display()));
    }
    let ancestor = git_ok(&main, &["merge-base", "--is-ancestor", stem, &trunk]);
    // A rebase leaves every commit with a patch-equivalent (`-`) on trunk.
    let equivalent = ancestor || git(&main, &["cherry", &trunk, stem]).is_ok_and(|c| !c.lines().any(|l| l.starts_with('+')));
    // A squash: merging the branch into trunk would change no file, so the merge tree is trunk's tree
    // (keep in sync with `has_own_commits` of `next_issue.rs`; git before 2.38 lacks `--write-tree` and counts as not merged).
    let merged = equivalent || {
        let tree = git(&main, &["merge-tree", "--write-tree", &trunk, stem]).ok().and_then(|t| t.lines().next().map(str::to_string));
        tree.is_some() && tree == git(&main, &["rev-parse", &format!("{trunk}^{{tree}}")]).ok()
    };
    if !merged && !force {
        return Err(format!("the branch {stem} is not merged into {trunk}: merge it first, or pass --force to drop it (it deletes work that is not on {trunk}; squash detection needs git 2.38); nothing was removed"));
    }
    match worktree_of(&main, stem) {
        Some(found) if found != dir => {
            return Err(format!("the branch {stem} is checked out in {}, not in {}: nothing was removed", found.display(), dir.display()));
        }
        Some(_) if !dir.exists() => {
            git(&main, &["worktree", "prune"])?;
        }
        Some(_) => {
            if !git(&dir, &["status", "--porcelain"])?.is_empty() {
                return Err(format!("{} has uncommitted changes: nothing was removed", dir.display()));
            }
            git(&main, &["worktree", "remove", &dir.to_string_lossy()])?;
        }
        None => {}
    }
    // `-d` only knows ancestors; the other cases were checked above or forced.
    git(&main, &["branch", if ancestor { "-d" } else { "-D" }, stem])?;
    Ok(format!("removed the worktree and the branch {stem}"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["start", issue] => start(Path::new("."), issue),
        ["finish", stem] => finish(Path::new("."), stem, false),
        ["finish", stem, "--force"] | ["finish", "--force", stem] => finish(Path::new("."), stem, true),
        _ => Err("usage: issue_worktree.rs start issues/<stem>.md | finish <stem> [--force]".to_string()),
    };
    match result {
        Ok(message) => println!("{message}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
