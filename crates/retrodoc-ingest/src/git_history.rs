//! Per-file Git history (modification frequency, authors, dates), used in
//! phase 2 to enrich the repo map. A single pass over the history
//! (`revwalk` + diff against the parent) feeds the map rather than a
//! `git log` per file, to stay practical on a large repo.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use git2::{Repository, Sort};

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
}
