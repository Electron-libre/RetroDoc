//! Per-file Git history (modification frequency, authors, dates), used in
//! phase 2 to enrich the repo map. A single pass over the history
//! (`revwalk` + diff against the parent) feeds the map rather than a
//! `git log` per file, to stay practical on a large repo.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use git2::{DiffOptions, Repository, Sort};

use crate::error::IngestError;

#[derive(Debug, Clone, Default)]
pub struct FileHistory {
    pub commit_count: u32,
    pub authors: HashSet<String>,
    pub first_commit_at: Option<DateTime<Utc>>,
    pub last_commit_at: Option<DateTime<Utc>>,
}

fn git_time_to_utc(time: git2::Time) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(time.seconds(), 0).single()
}

/// Hash of the commit `HEAD` points to; `None` for a repo without commits
/// (or not a repo).
#[must_use]
pub fn head_commit(repo_root: &Path) -> Option<String> {
    let repo = Repository::open(repo_root).ok()?;
    let head = repo.head().ok()?.peel_to_commit().ok()?;
    Some(head.id().to_string())
}

/// Rebuilds, for each file touched in `HEAD`'s history, the commit count,
/// authors, and first/last modification dates.
///
/// # Errors
///
/// Returns an error if the repo can't be opened or reading the history
/// fails (corrupted repo, disk access).
pub fn collect_history(repo_root: &Path) -> Result<HashMap<PathBuf, FileHistory>, IngestError> {
    let repo = Repository::open(repo_root)?;
    let mut history: HashMap<PathBuf, FileHistory> = HashMap::new();

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(Sort::TIME)?;
    match revwalk.push_head() {
        Ok(()) => {}
        // Repo with no commits (just initialized): empty history, not an error.
        Err(_) => return Ok(history),
    }

    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };

        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
        let author = commit.author();
        let author_label = author.name().map_or_else(
            || author.email().unwrap_or("unknown").to_string(),
            str::to_string,
        );
        let when = git_time_to_utc(commit.time());

        diff.foreach(
            &mut |delta, _progress| {
                let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) else {
                    return true;
                };
                let entry = history.entry(path.to_path_buf()).or_default();
                entry.commit_count += 1;
                entry.authors.insert(author_label.clone());
                if let Some(when) = when {
                    entry.first_commit_at = Some(match entry.first_commit_at {
                        Some(existing) => existing.min(when),
                        None => when,
                    });
                    entry.last_commit_at = Some(match entry.last_commit_at {
                        Some(existing) => existing.max(when),
                        None => when,
                    });
                }
                true
            },
            None,
            None,
            None,
        )?;
    }

    Ok(history)
}

/// A commit of a file's history, as shown to an agent: no author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    /// First 8 characters of the commit hash.
    pub short_id: String,
    pub committed_at: Option<DateTime<Utc>>,
    /// First line of the message.
    pub subject: String,
}

/// The `limit` most recent commits of `HEAD`'s history that changed the file
/// `path` (relative to the repo root, taken literally, no glob), newest
/// first, merge commits left out. A file that was renamed is followed no
/// further than its current name; an unknown file or a repo without commits gives an empty list.
///
/// # Errors
///
/// Returns an error if the repo can't be opened or reading the history
/// fails.
pub fn file_log(
    repo_root: &Path,
    path: &Path,
    limit: usize,
) -> Result<Vec<CommitSummary>, IngestError> {
    let repo = Repository::open(repo_root)?;
    let mut log = Vec::new();
    let mut revwalk = repo.revwalk()?;
    // Topological first: commits made within the same second keep their order.
    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    if limit == 0 || revwalk.push_head().is_err() {
        return Ok(log);
    }
    let mut options = DiffOptions::new();
    options.pathspec(path).disable_pathspec_match(true);

    for oid in revwalk {
        let commit = repo.find_commit(oid?)?;
        // The commits a merge brings in are walked on their own: the merge
        // itself is not a change of the file (as in `git log --no-merges`).
        if commit.parent_count() > 1 {
            continue;
        }
        let tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };
        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut options))?;
        if diff.deltas().len() == 0 {
            continue;
        }
        log.push(CommitSummary {
            short_id: commit.id().to_string().chars().take(8).collect(),
            committed_at: git_time_to_utc(commit.time()),
            subject: commit
                .message()
                .and_then(|m| m.lines().next())
                .unwrap_or_default()
                .to_string(),
        });
        if log.len() == limit {
            break;
        }
    }
    Ok(log)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn run_git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test Author")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test Author")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .expect("git command failed to run");
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn collects_commit_count_and_authors_per_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        fs::write(root.join("a.txt"), "1").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "-q", "-m", "first"]);
        fs::write(root.join("a.txt"), "2").unwrap();
        fs::write(root.join("b.txt"), "1").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "-q", "-m", "second"]);

        let history = collect_history(root).unwrap();
        let a = history.get(Path::new("a.txt")).unwrap();
        assert_eq!(a.commit_count, 2);
        assert_eq!(a.authors.len(), 1);
        assert!(a.first_commit_at.unwrap() <= a.last_commit_at.unwrap());

        let b = history.get(Path::new("b.txt")).unwrap();
        assert_eq!(b.commit_count, 1);
    }

    #[test]
    fn repo_without_commits_returns_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        let history = collect_history(root).unwrap();
        assert!(history.is_empty());
    }

    fn commit_file(root: &Path, name: &str, content: &str, message: &str) {
        fs::write(root.join(name), content).unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "-q", "-m", message]);
    }

    #[test]
    fn the_log_of_a_file_lists_its_commits_newest_first_up_to_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        commit_file(root, "a.txt", "1", "add a\n\nlong body");
        commit_file(root, "b.txt", "1", "add b");
        commit_file(root, "a.txt", "2", "change a");
        commit_file(root, "a.txt", "3", "change a again");

        let log = file_log(root, Path::new("a.txt"), 10).unwrap();
        let subjects: Vec<&str> = log.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, ["change a again", "change a", "add a"]);
        assert_eq!(log[0].short_id.len(), 8);
        assert!(log[0].committed_at.is_some());

        let log = file_log(root, Path::new("a.txt"), 2).unwrap();
        assert_eq!(log.len(), 2);
        let log = file_log(root, Path::new("b.txt"), 10).unwrap();
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn an_unknown_file_or_a_repo_without_commit_has_an_empty_log() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        assert_eq!(file_log(root, Path::new("a.txt"), 5).unwrap(), vec![]);
        commit_file(root, "a.txt", "1", "add a");
        assert_eq!(file_log(root, Path::new("nope.txt"), 5).unwrap(), vec![]);
    }

    #[test]
    fn the_path_is_literal_so_a_glob_character_matches_only_that_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run_git(root, &["init", "-q"]);
        commit_file(root, "[id].tsx", "1", "add the page");
        commit_file(root, "i.tsx", "1", "add another page");
        let log = file_log(root, Path::new("[id].tsx"), 5).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].subject, "add the page");
        let log = file_log(root, Path::new("*.tsx"), 5).unwrap();
        assert_eq!(log, vec![]);
    }

    #[test]
    fn a_directory_without_repo_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(file_log(dir.path(), Path::new("a.txt"), 5).is_err());
    }

    #[test]
    fn a_merge_commit_is_not_listed_as_a_change_of_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run_git(root, &["init", "-q", "-b", "main"]);
        commit_file(root, "a.txt", "1", "add a");
        run_git(root, &["checkout", "-q", "-b", "feature"]);
        commit_file(root, "a.txt", "2", "change a on the branch");
        run_git(root, &["checkout", "-q", "main"]);
        commit_file(root, "b.txt", "1", "add b on main");
        run_git(
            root,
            &[
                "merge",
                "-q",
                "--no-ff",
                "feature",
                "-m",
                "Merge branch feature",
            ],
        );

        let log = file_log(root, Path::new("a.txt"), 10).unwrap();
        let subjects: Vec<&str> = log.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, ["change a on the branch", "add a"]);
    }
}
