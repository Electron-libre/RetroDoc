//! `read_source` and `git_log`: the code behind the docs, for an agent that
//! wants to check a claim. Deliberately narrow: only the files the docs cite
//! (a feature's source paths, a step's source refs) are reachable, never an
//! arbitrary path, and no symbolic link followed. Reads are
//! windowed by lines, and binary or very large files are refused.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

/// Lines returned when the agent gives no range, and the most one call returns.
const DEFAULT_LINES: usize = 200;
const MAX_LINES: usize = 400;
/// Larger files are refused: they are data or generated code, not what a doc cites.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Characters of one line shown: a minified bundle is one line of hundreds of KB.
const MAX_LINE_CHARS: usize = 400;
/// Commits `git_log` returns when the agent gives no limit, and the most it can ask for.
pub const DEFAULT_LOG: usize = 10;
pub const MAX_LOG: usize = 50;

/// What `read_source` returns: a header line, then the numbered code.
#[derive(Debug, PartialEq, Eq)]
pub struct Excerpt {
    pub header: String,
    pub body: String,
}

/// A window of the lines of a cited file.
#[derive(Debug, PartialEq, Eq)]
pub struct Window {
    /// The first and last lines shown (1-based, inclusive); `0` and `0` for an empty file.
    pub start: usize,
    pub end: usize,
    /// The lines of the whole file.
    pub total: usize,
    pub lines: Vec<Line>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Line {
    pub number: usize,
    pub text: String,
    /// The length in characters of a line cut to [`MAX_LINE_CHARS`].
    pub cut_from: Option<usize>,
}

/// A commit that changed a cited file: no author, on purpose.
#[derive(Debug, PartialEq, Eq)]
pub struct FileCommit {
    pub short_id: String,
    /// `YYYY-MM-DD`, or `unknown date`.
    pub date: String,
    pub subject: String,
    /// The path the file had before this commit renamed it.
    pub renamed_from: Option<String>,
}

/// Why a file can't be shown.
#[derive(Debug, PartialEq, Eq)]
pub enum SourceError {
    /// Absolute, empty, or with a `..`.
    NotPlain(String),
    NotCited(String),
    Symlink(String),
    Gone(String),
    Unreadable(String),
    TooLarge {
        path: String,
        size: u64,
    },
    Binary(String),
    PastEnd {
        path: String,
        total: usize,
        start: usize,
    },
    Reversed {
        start: usize,
        end: usize,
    },
    NoHistory,
}

impl SourceError {
    /// The path is none the docs cite: whether it exists is not told.
    #[must_use]
    pub fn not_served(&self) -> bool {
        matches!(self, Self::NotPlain(_) | Self::NotCited(_))
    }
}

impl std::fmt::Display for SourceError {
    /// As a sentence for the agent.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPlain(path) => write!(
                f,
                "Not available: `{path}` is not a path inside the repository (no absolute path, no `..`)."
            ),
            Self::NotCited(path) => write!(
                f,
                "Not available: `{path}` is not cited by the docs, and only cited files can be read. \
                 Find the files of a feature or use case with `get_feature` or `get_use_case`."
            ),
            Self::Symlink(path) => write!(
                f,
                "Not available: `{path}` is, or goes through, a symbolic link, and links are not followed."
            ),
            Self::Gone(path) => write!(f, "Not available: `{path}` no longer exists in the repository."),
            Self::Unreadable(path) => write!(f, "Not available: `{path}` could not be read."),
            Self::TooLarge { path, size } => write!(
                f,
                "Not available: `{path}` is too large to show ({size} bytes, the limit is {MAX_FILE_BYTES})."
            ),
            Self::Binary(path) => write!(f, "Not available: `{path}` is a binary file."),
            Self::PastEnd { path, total, start } => write!(
                f,
                "`{path}` has {total} lines: start_line {start} is past the end."
            ),
            Self::Reversed { start, end } => {
                write!(f, "end_line ({end}) is before start_line ({start}).")
            }
            Self::NoHistory => write!(
                f,
                "Not available: this repository has no readable git history."
            ),
        }
    }
}

pub struct SourceAccess {
    repo_root: PathBuf,
    cited: BTreeSet<String>,
}

impl SourceAccess {
    /// `cited`: the repo-relative paths the docs cite.
    #[must_use]
    pub fn new(repo_root: &Path, cited: impl IntoIterator<Item = String>) -> Self {
        Self {
            repo_root: repo_root.to_path_buf(),
            // `./a.rb` and `a.rb` are the same citation.
            cited: cited
                .into_iter()
                .map(|path| path.strip_prefix("./").unwrap_or(&path).to_string())
                .collect(),
        }
    }

    /// A window of the cited file `path`, lines `start_line` to `end_line`
    /// (1-based, inclusive).
    ///
    /// # Errors
    ///
    /// Returns the reason, as a sentence for the agent, when the file can't
    /// be shown: not cited, outside the repo, gone, binary, too large, or a
    /// range past its end.
    pub fn read_source(
        &self,
        path: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> Result<Excerpt, String> {
        let window = self
            .window(path, start_line, end_line)
            .map_err(|e| e.to_string())?;
        let path = path.strip_prefix("./").unwrap_or(path);
        if window.total == 0 {
            return Ok(Excerpt {
                header: format!("# `{path}` (empty file)"),
                body: String::new(),
            });
        }
        let width = window.total.to_string().len().max(4);
        let mut shown = String::new();
        for line in &window.lines {
            let (number, text) = (line.number, &line.text);
            match line.cut_from {
                Some(length) => {
                    let _ = writeln!(
                        shown,
                        "{number:>width$} | {text} … (line cut, {length} characters)"
                    );
                }
                None => {
                    let _ = writeln!(shown, "{number:>width$} | {text}");
                }
            }
        }
        let longest_backticks = shown
            .split(|c| c != '`')
            .map(str::len)
            .max()
            .unwrap_or_default();
        let fence = "`".repeat((longest_backticks + 1).max(3));
        let mut body = format!("{fence}\n{shown}{fence}\n");
        if window.end < window.total {
            let _ = write!(
                body,
                "\nMore: call `read_source` with `start_line={}`.\n",
                window.end + 1
            );
        }
        Ok(Excerpt {
            header: format!(
                "# `{path}` (lines {}-{} of {})",
                window.start, window.end, window.total
            ),
            body,
        })
    }

    /// The lines `start_line` to `end_line` (1-based, inclusive) of the cited
    /// file `path`: [`DEFAULT_LINES`] from `start_line` with no end, at most
    /// [`MAX_LINES`], each cut to [`MAX_LINE_CHARS`].
    ///
    /// # Errors
    ///
    /// When the file can't be shown: not cited, outside the repo, gone,
    /// binary, too large, or a range past its end.
    pub fn window(
        &self,
        path: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> Result<Window, SourceError> {
        let path = self.check_cited(path)?;
        let real = self.resolve_inside_repo(path)?;
        let size = std::fs::metadata(&real)
            .map_err(|e| unreadable(path, &e))?
            .len();
        if size > MAX_FILE_BYTES {
            return Err(SourceError::TooLarge {
                path: path.to_string(),
                size,
            });
        }
        let bytes = std::fs::read(&real).map_err(|e| unreadable(path, &e))?;
        if bytes.contains(&0) {
            return Err(SourceError::Binary(path.to_string()));
        }
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();
        if total == 0 {
            return Ok(Window {
                start: 0,
                end: 0,
                total,
                lines: Vec::new(),
            });
        }

        let start = start_line.unwrap_or(1).max(1);
        if start > total {
            return Err(SourceError::PastEnd {
                path: path.to_string(),
                total,
                start,
            });
        }
        if let Some(end) = end_line.filter(|end| *end < start) {
            return Err(SourceError::Reversed { start, end });
        }
        let end = end_line
            .map_or(start + DEFAULT_LINES - 1, |end| {
                end.min(start + MAX_LINES - 1)
            })
            .min(total);
        let lines = lines[start - 1..end]
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let length = line.chars().count();
                let cut = length > MAX_LINE_CHARS;
                Line {
                    number: start + index,
                    text: if cut {
                        line.chars().take(MAX_LINE_CHARS).collect()
                    } else {
                        (*line).to_string()
                    },
                    cut_from: cut.then_some(length),
                }
            })
            .collect();
        Ok(Window {
            start,
            end,
            total,
            lines,
        })
    }

    /// The last `limit` commits that changed the cited file `path`, across
    /// its renames: short hash, date, subject and the name a rename came
    /// from, no author.
    ///
    /// # Errors
    ///
    /// Returns the reason, as a sentence for the agent, when the history
    /// can't be shown: file not cited, no git repository.
    pub fn git_log(&self, path: &str, limit: usize) -> Result<String, String> {
        let log = self.commits(path, limit).map_err(|e| e.to_string())?;
        let path = path.strip_prefix("./").unwrap_or(path);
        if log.is_empty() {
            return Ok(format!(
                "# History of `{path}`\n\nNo commit touches this file (not committed yet?).\n"
            ));
        }
        let mut out = format!("# History of `{path}` (last {} commit(s))\n\n", log.len());
        for commit in log {
            let _ = write!(
                out,
                "- `{}` · {} · {}",
                commit.short_id, commit.date, commit.subject
            );
            if let Some(old_path) = &commit.renamed_from {
                let _ = write!(out, " (renamed from `{old_path}`)");
            }
            out.push('\n');
        }
        Ok(out)
    }

    /// The last `limit` commits (at most [`MAX_LOG`]) that changed the cited
    /// file `path`, newest first.
    ///
    /// # Errors
    ///
    /// When the file is not cited or the repository has no readable history.
    pub fn commits(&self, path: &str, limit: usize) -> Result<Vec<FileCommit>, SourceError> {
        let path = self.check_cited(path)?;
        let limit = limit.clamp(1, MAX_LOG);
        let log = retrodoc_ingest::git_history::file_log(&self.repo_root, Path::new(path), limit)
            .map_err(|error| {
            tracing::warn!(%error, path, "git_log could not read the history");
            SourceError::NoHistory
        })?;
        Ok(log
            .into_iter()
            .map(|commit| FileCommit {
                short_id: commit.short_id,
                date: commit.committed_at.map_or_else(
                    || "unknown date".to_string(),
                    |d| d.format("%Y-%m-%d").to_string(),
                ),
                subject: commit.subject,
                renamed_from: commit
                    .renamed_from
                    .map(|path| path.to_string_lossy().into_owned()),
            })
            .collect())
    }

    /// The path as the docs cite it, if it is a plain repo-relative path of a
    /// cited file; else why not.
    fn check_cited<'a>(&self, path: &'a str) -> Result<&'a str, SourceError> {
        let path = path.strip_prefix("./").unwrap_or(path);
        let plain = !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)));
        if !plain {
            return Err(SourceError::NotPlain(path.to_string()));
        }
        if !self.cited.contains(path) {
            return Err(SourceError::NotCited(path.to_string()));
        }
        Ok(path)
    }

    /// The real location of `path`. A symbolic link anywhere on the way is
    /// refused, whatever it points to: it could lead to a file the docs don't
    /// cite, or out of the repo.
    fn resolve_inside_repo(&self, path: &str) -> Result<PathBuf, SourceError> {
        let real = self
            .repo_root
            .join(path)
            .canonicalize()
            .map_err(|e| unreadable(path, &e))?;
        let root = self
            .repo_root
            .canonicalize()
            .map_err(|e| unreadable(path, &e))?;
        if real == root.join(path) {
            Ok(real)
        } else {
            Err(SourceError::Symlink(path.to_string()))
        }
    }
}

fn unreadable(path: &str, error: &std::io::Error) -> SourceError {
    if error.kind() == std::io::ErrorKind::NotFound {
        SourceError::Gone(path.to_string())
    } else {
        SourceError::Unreadable(path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Ada Lovelace")
            .env("GIT_AUTHOR_EMAIL", "ada@example.com")
            .env("GIT_COMMITTER_NAME", "Ada Lovelace")
            .env("GIT_COMMITTER_EMAIL", "ada@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn commit(root: &Path, name: &str, content: &str, message: &str) {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", message]);
    }

    /// A git repo (with a sibling secret outside it) where the docs cite
    /// `app/invoice.rb` only.
    fn repo() -> (tempfile::TempDir, SourceAccess) {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(outer.path().join("secret.txt"), "top secret").unwrap();
        git(&root, &["init", "-q"]);
        let mut code = String::new();
        for n in 1..=500 {
            let _ = writeln!(code, "line {n}");
        }
        commit(&root, "app/invoice.rb", &code, "add invoices");
        commit(&root, "app/other.rb", "x\n", "add other");
        commit(
            &root,
            "app/invoice.rb",
            &format!("{code}more\n"),
            "extend invoices\n\nlong body",
        );
        let access = SourceAccess::new(&root, ["app/invoice.rb".to_string()]);
        (outer, access)
    }

    #[test]
    fn a_cited_file_is_shown_with_line_numbers_from_the_top_by_default() {
        let (_dir, access) = repo();
        let excerpt = access.read_source("app/invoice.rb", None, None).unwrap();
        assert_eq!(excerpt.header, "# `app/invoice.rb` (lines 1-200 of 501)");
        assert!(excerpt.body.contains("   1 | line 1\n"), "{}", excerpt.body);
        assert!(excerpt.body.contains(" 200 | line 200\n"));
        assert!(!excerpt.body.contains("line 201"));
        assert!(excerpt.body.contains("start_line=201"), "{}", excerpt.body);
    }

    #[test]
    fn a_range_is_honoured_and_capped() {
        let (_dir, access) = repo();
        let excerpt = access
            .read_source("app/invoice.rb", Some(10), Some(12))
            .unwrap();
        assert_eq!(excerpt.header, "# `app/invoice.rb` (lines 10-12 of 501)");
        assert!(
            excerpt.body.contains("  10 | line 10\n") && excerpt.body.contains("  12 | line 12\n")
        );
        assert!(!excerpt.body.contains("line 13"));
        let wide = access
            .read_source("app/invoice.rb", Some(1), Some(500))
            .unwrap();
        assert_eq!(wide.header, "# `app/invoice.rb` (lines 1-400 of 501)");
        let end = access
            .read_source("app/invoice.rb", Some(499), Some(900))
            .unwrap();
        assert_eq!(end.header, "# `app/invoice.rb` (lines 499-501 of 501)");
        assert!(!end.body.contains("start_line="));
    }

    #[test]
    fn a_range_past_the_end_or_reversed_is_explained() {
        let (_dir, access) = repo();
        let err = access
            .read_source("app/invoice.rb", Some(900), None)
            .unwrap_err();
        assert!(err.contains("501 lines"), "{err}");
        let err = access
            .read_source("app/invoice.rb", Some(20), Some(10))
            .unwrap_err();
        assert!(err.contains("end_line"), "{err}");
    }

    #[test]
    fn a_file_the_docs_do_not_cite_is_refused_even_if_it_exists() {
        let (_dir, access) = repo();
        let err = access.read_source("app/other.rb", None, None).unwrap_err();
        assert!(
            err.starts_with("Not available:") && err.contains("not cited"),
            "{err}"
        );
        assert!(access
            .git_log("app/other.rb", 5)
            .unwrap_err()
            .contains("not cited"));
    }

    #[test]
    fn paths_leading_outside_the_repo_are_refused_even_when_cited() {
        let (dir, _) = repo();
        let root = dir.path().join("repo");
        let hostile = ["../secret.txt", "app/../../secret.txt", "/etc/passwd", ""];
        let access = SourceAccess::new(&root, hostile.map(String::from));
        for path in hostile {
            let err = access.read_source(path, None, None).unwrap_err();
            assert!(err.starts_with("Not available:"), "{path}: {err}");
            assert!(!err.contains("top secret"));
            assert!(access.git_log(path, 5).is_err(), "{path}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_cited_symlink_pointing_outside_the_repo_is_refused() {
        let (dir, _) = repo();
        let root = dir.path().join("repo");
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), root.join("app/link.rb"))
            .unwrap();
        let access = SourceAccess::new(&root, ["app/link.rb".to_string()]);
        let err = access.read_source("app/link.rb", None, None).unwrap_err();
        assert!(err.contains("symbolic link"), "{err}");
        assert!(!err.contains("top secret"));
    }

    #[cfg(unix)]
    #[test]
    fn a_cited_symlink_to_an_uncited_file_of_the_repo_does_not_open_it() {
        let (dir, _) = repo();
        let root = dir.path().join("repo");
        std::fs::write(root.join("secrets.yml"), "password: hunter2\n").unwrap();
        std::os::unix::fs::symlink("secrets.yml", root.join("current.yml")).unwrap();
        std::fs::create_dir_all(root.join("real")).unwrap();
        std::fs::write(root.join("real/a.rb"), "x\n").unwrap();
        std::os::unix::fs::symlink("real", root.join("alias")).unwrap();
        let access = SourceAccess::new(&root, ["current.yml", "alias/a.rb"].map(String::from));
        for path in ["current.yml", "alias/a.rb"] {
            let err = access.read_source(path, None, None).unwrap_err();
            assert!(err.contains("symbolic link"), "{path}: {err}");
            assert!(!err.contains("hunter2"));
        }
    }

    #[test]
    fn a_cited_path_written_with_a_leading_dot_slash_is_still_reachable() {
        let (dir, _) = repo();
        let root = dir.path().join("repo");
        let access = SourceAccess::new(&root, ["./app/invoice.rb".to_string()]);
        assert!(access.read_source("app/invoice.rb", None, None).is_ok());
        assert!(access.read_source("./app/invoice.rb", None, None).is_ok());
        assert!(access.git_log("./app/invoice.rb", 3).is_ok());
    }

    #[test]
    fn a_very_long_line_is_cut_and_a_late_nul_byte_still_means_binary() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("min.js"), "x".repeat(900_000)).unwrap();
        let mut late = vec![b'a'; 20_000];
        late.push(0);
        std::fs::write(dir.path().join("late.bin"), late).unwrap();
        let access = SourceAccess::new(dir.path(), ["min.js", "late.bin"].map(String::from));
        let excerpt = access.read_source("min.js", None, None).unwrap();
        assert!(excerpt.body.len() < 2_000, "{} bytes", excerpt.body.len());
        assert!(excerpt.body.contains("line cut"), "{}", excerpt.body);
        let err = access.read_source("late.bin", None, None).unwrap_err();
        assert!(err.contains("binary"), "{err}");
    }

    #[test]
    fn binary_large_and_vanished_files_are_refused_with_a_reason() {
        let (dir, _) = repo();
        let root = dir.path().join("repo");
        std::fs::write(root.join("logo.bin"), [0_u8, 1, 2, 0]).unwrap();
        std::fs::write(root.join("big.sql"), vec![b'a'; 1024 * 1024 + 1]).unwrap();
        let access = SourceAccess::new(&root, ["logo.bin", "big.sql", "gone.rb"].map(String::from));
        assert!(access
            .read_source("logo.bin", None, None)
            .unwrap_err()
            .contains("binary"));
        assert!(access
            .read_source("big.sql", None, None)
            .unwrap_err()
            .contains("too large"));
        assert!(access
            .read_source("gone.rb", None, None)
            .unwrap_err()
            .contains("no longer exists"));
    }

    #[test]
    fn the_history_shows_hash_date_and_subject_but_no_author() {
        let (_dir, access) = repo();
        let log = access.git_log("app/invoice.rb", 10).unwrap();
        assert!(log.starts_with("# History of `app/invoice.rb`"), "{log}");
        assert!(log.contains(" · extend invoices\n"), "{log}");
        assert!(log.contains(" · add invoices\n"), "{log}");
        assert!(!log.contains("add other"), "{log}");
        assert!(!log.contains("long body"), "{log}");
        assert!(
            !log.contains("Ada") && !log.contains("example.com"),
            "{log}"
        );
        let newest = log.find("extend invoices").unwrap();
        assert!(newest < log.find("add invoices").unwrap());
        let one = access.git_log("app/invoice.rb", 1).unwrap();
        assert!(!one.contains("add invoices"), "{one}");
    }

    #[test]
    fn a_folder_that_is_not_a_git_repo_has_no_history() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rb"), "x").unwrap();
        let access = SourceAccess::new(dir.path(), ["a.rb".to_string()]);
        let err = access.git_log("a.rb", 5).unwrap_err();
        assert!(
            err.starts_with("Not available:") && err.contains("git"),
            "{err}"
        );
    }
}
