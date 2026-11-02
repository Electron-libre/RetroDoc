//! Comparison of the rendered files with the docs dir: what would be
//! created, updated, removed or left alone, and the diff of each update.
//!
//! The pages written under `functional/` are recorded with the hash of their
//! content (`RENDERED_PAGES_PATH`): a page no run produces any more is removed
//! when it still has its recorded hash, and kept and reported otherwise, as it
//! may hold hand-written content (`specs/docs-pruning.md`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use similar::TextDiff;

use crate::{RenderError, RenderedFile, FUNCTIONAL_DIR, METADATA_PATH, RENDERED_PAGES_PATH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Created,
    /// Differs from the file on disk; carries the unified diff.
    Updated(String),
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct PlannedFile {
    pub file: RenderedFile,
    pub status: FileStatus,
}

#[derive(Debug, Clone, Default)]
pub struct WritePlan {
    pub files: Vec<PlannedFile>,
    /// Markdown files under `functional/` that this run no longer produces
    /// and that still have the hash recorded when they were written: removed
    /// by [`apply`](Self::apply).
    pub removed: Vec<PathBuf>,
    /// Markdown files under `functional/` that this run no longer produces
    /// (a feature renamed or gone) and that were edited since they were
    /// written, or never recorded. Reported, never deleted: they may hold
    /// hand-written content.
    pub stale: Vec<PathBuf>,
}

/// Compares `files` (paths relative to `docs_dir`) with what is on disk, and
/// adds the record of the pages they write under `functional/`.
///
/// # Errors
///
/// Returns an error if an existing file or folder can't be read.
pub fn plan(docs_dir: &Path, mut files: Vec<RenderedFile>) -> Result<WritePlan, RenderError> {
    let recorded = read_record(docs_dir);
    files.push(record_of(&files));

    let mut planned = Vec::with_capacity(files.len());
    for file in files {
        let abs = docs_dir.join(&file.path);
        let status = match std::fs::read_to_string(&abs) {
            Ok(old) if old == file.content => FileStatus::Unchanged,
            Ok(old) => FileStatus::Updated(unified_diff(&file.path, &old, &file.content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileStatus::Created,
            Err(source) => return Err(RenderError::Read { path: abs, source }),
        };
        planned.push(PlannedFile { file, status });
    }

    // The metadata carries a timestamp: when it is the only thing that
    // changed, and so is the only file to rewrite, leave it alone.
    let only_metadata_differs = planned
        .iter()
        .filter(|p| p.status != FileStatus::Unchanged)
        .all(|p| p.file.path == Path::new(METADATA_PATH));
    if only_metadata_differs {
        for p in &mut planned {
            if p.file.path == Path::new(METADATA_PATH) && same_but_timestamp(docs_dir, &p.file) {
                p.status = FileStatus::Unchanged;
            }
        }
    }

    let produced: BTreeSet<&Path> = planned.iter().map(|p| p.file.path.as_path()).collect();
    let mut not_produced = Vec::new();
    collect_markdown(
        &docs_dir.join(FUNCTIONAL_DIR),
        docs_dir,
        &produced,
        &mut not_produced,
    )?;
    not_produced.sort();
    let (removed, stale) = not_produced.into_iter().partition(|path| {
        recorded.get(&record_key(path)).is_some_and(|hash| {
            std::fs::read(docs_dir.join(path)).is_ok_and(|content| *hash == hash_of(&content))
        })
    });

    Ok(WritePlan {
        files: planned,
        removed,
        stale,
    })
}

/// The record of the Markdown pages `files` writes under `functional/`: path
/// to hash of the content, sorted, so the same pages give the same bytes.
fn record_of(files: &[RenderedFile]) -> RenderedFile {
    let record: BTreeMap<String, String> = files
        .iter()
        .filter(|f| is_functional_page(&f.path))
        .map(|f| (record_key(&f.path), hash_of(f.content.as_bytes())))
        .collect();
    let content = serde_json::to_string_pretty(&record).expect("a map of strings serializes");
    RenderedFile {
        path: PathBuf::from(RENDERED_PAGES_PATH),
        content: content + "\n",
    }
}

/// The record of the last run; empty when there is none or it can't be read,
/// so that no page is removed.
fn read_record(docs_dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(docs_dir.join(RENDERED_PAGES_PATH))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn is_functional_page(path: &Path) -> bool {
    path.starts_with(FUNCTIONAL_DIR) && path.extension().is_some_and(|e| e == "md")
}

/// A path of the record, with `/` on every platform.
fn record_key(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn hash_of(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

fn same_but_timestamp(docs_dir: &Path, file: &RenderedFile) -> bool {
    let strip = |raw: &str| {
        let mut value: serde_json::Value = serde_json::from_str(raw).ok()?;
        value.as_object_mut()?.remove("generated_at");
        Some(value)
    };
    let Ok(old) = std::fs::read_to_string(docs_dir.join(&file.path)) else {
        return false;
    };
    matches!((strip(&old), strip(&file.content)), (Some(a), Some(b)) if a == b)
}

fn collect_markdown(
    dir: &Path,
    docs_dir: &Path,
    produced: &BTreeSet<&Path>,
    out: &mut Vec<PathBuf>,
) -> Result<(), RenderError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(RenderError::Read {
                path: dir.to_path_buf(),
                source,
            })
        }
    };
    for entry in entries {
        let path = entry
            .map_err(|source| RenderError::Read {
                path: dir.to_path_buf(),
                source,
            })?
            .path();
        if path.is_dir() {
            collect_markdown(&path, docs_dir, produced, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            if let Ok(rel) = path.strip_prefix(docs_dir) {
                if !produced.contains(rel) {
                    out.push(rel.to_path_buf());
                }
            }
        }
    }
    Ok(())
}

fn unified_diff(path: &Path, old: &str, new: &str) -> String {
    let name = path.display();
    TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{name}"), &format!("b/{name}"))
        .to_string()
}

impl WritePlan {
    /// Number of files that [`apply`](Self::apply) would write.
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.files
            .iter()
            .filter(|p| p.status != FileStatus::Unchanged)
            .count()
    }

    /// One line per file that changes or is removed, then the diffs of the
    /// updates, then the stale files; nothing but a one-line notice when all
    /// is up to date.
    #[must_use]
    pub fn preview(&self) -> String {
        let mut out = String::new();
        for p in &self.files {
            let label = match p.status {
                FileStatus::Created => "create",
                FileStatus::Updated(_) => "update",
                FileStatus::Unchanged => continue,
            };
            let _ = writeln!(out, "{label}  {}", p.file.path.display());
        }
        for path in &self.removed {
            let _ = writeln!(out, "remove  {}", path.display());
        }
        if out.is_empty() {
            out.push_str("Docs are up to date, nothing to write.\n");
        }
        for p in &self.files {
            if let FileStatus::Updated(diff) = &p.status {
                let _ = write!(out, "\n{diff}");
            }
        }
        if !self.stale.is_empty() {
            out.push_str(
                "\nNot produced by this run (left in place, delete by hand if obsolete):\n",
            );
            for path in &self.stale {
                let _ = writeln!(out, "  {}", path.display());
            }
        }
        out
    }

    /// Writes the created and updated files under `docs_dir` and removes the
    /// [`removed`](Self::removed) ones, with the folders under `functional/`
    /// they leave empty; returns how many files were written. Unchanged files
    /// are not touched (mtime included).
    ///
    /// # Errors
    ///
    /// Returns an error if a folder can't be created, a file written or a
    /// file removed.
    pub fn apply(&self, docs_dir: &Path) -> Result<usize, RenderError> {
        let mut written = 0;
        for p in &self.files {
            if p.status == FileStatus::Unchanged {
                continue;
            }
            let abs = docs_dir.join(&p.file.path);
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent).map_err(|source| RenderError::Write {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
            std::fs::write(&abs, &p.file.content)
                .map_err(|source| RenderError::Write { path: abs, source })?;
            written += 1;
        }
        for path in &self.removed {
            let abs = docs_dir.join(path);
            std::fs::remove_file(&abs).map_err(|source| RenderError::Remove {
                path: abs.clone(),
                source,
            })?;
            remove_empty_parents(&abs, &docs_dir.join(FUNCTIONAL_DIR));
        }
        Ok(written)
    }
}

/// Removes the folders of `file` left empty, up to `root` excluded; stops at
/// the first one that still holds something.
fn remove_empty_parents(file: &Path, root: &Path) {
    let mut dir = file.parent();
    while let Some(d) = dir.filter(|d| d.starts_with(root) && *d != root) {
        // Fails on a folder that is not empty: the end of the walk.
        if std::fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, content: &str) -> RenderedFile {
        RenderedFile {
            path: PathBuf::from(path),
            content: content.to_string(),
        }
    }

    #[test]
    fn second_apply_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let files = vec![file("functional/a/README.md", "# A\n")];

        let first = plan(dir.path(), files.clone()).unwrap();
        assert_eq!(first.files[0].status, FileStatus::Created);
        // The page and the record of its hash.
        assert_eq!(first.apply(dir.path()).unwrap(), 2);

        let second = plan(dir.path(), files).unwrap();
        assert_eq!(second.change_count(), 0);
        assert!(second.preview().contains("up to date"));
    }

    #[test]
    fn update_carries_a_diff_and_dry_run_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("functional")).unwrap();
        std::fs::write(dir.path().join("functional/a.md"), "one\ntwo\n").unwrap();

        let planned = plan(dir.path(), vec![file("functional/a.md", "one\n2\n")]).unwrap();
        let preview = planned.preview();
        assert!(preview.contains("update  functional/a.md"));
        assert!(preview.contains("-two") && preview.contains("+2"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("functional/a.md")).unwrap(),
            "one\ntwo\n"
        );
    }

    #[test]
    fn timestamp_only_change_of_metadata_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"{"model":"m","commit":"c","generated_at":"2026-01-01T00:00:00Z"}"#;
        let new = r#"{"model":"m","commit":"c","generated_at":"2026-02-02T00:00:00Z"}"#;
        plan(dir.path(), vec![file(METADATA_PATH, old)])
            .unwrap()
            .apply(dir.path())
            .unwrap();

        let same = plan(dir.path(), vec![file(METADATA_PATH, new)]).unwrap();
        assert_eq!(same.change_count(), 0);

        // Another model: a real change.
        let other = new.replace("\"m\"", "\"n\"");
        let changed = plan(dir.path(), vec![file(METADATA_PATH, &other)]).unwrap();
        assert_eq!(changed.change_count(), 1);

        // Other docs change too: the timestamp is refreshed along.
        let with_doc = plan(
            dir.path(),
            vec![file(METADATA_PATH, new), file("functional/x.md", "x")],
        )
        .unwrap();
        // The metadata, the page and the record of its hash.
        assert_eq!(with_doc.change_count(), 3);
    }

    #[test]
    fn reports_stale_markdown_without_deleting_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("functional/old")).unwrap();
        std::fs::write(dir.path().join("functional/old/gone.md"), "x").unwrap();

        let planned = plan(dir.path(), vec![file("functional/new.md", "y")]).unwrap();
        assert_eq!(planned.stale, vec![PathBuf::from("functional/old/gone.md")]);
        planned.apply(dir.path()).unwrap();
        assert!(dir.path().join("functional/old/gone.md").exists());
    }

    // spec: RND-2, RND-3, RND-4 (internal: a folder that keeps a page, which the CLI tests can't build from artifacts)
    #[test]
    fn removes_a_recorded_page_and_keeps_its_folder_while_it_holds_another() {
        let dir = tempfile::tempdir().unwrap();
        let old = vec![
            file("functional/old/README.md", "a"),
            file("functional/old/edited.md", "b"),
        ];
        plan(dir.path(), old).unwrap().apply(dir.path()).unwrap();
        std::fs::write(dir.path().join("functional/old/edited.md"), "b by hand").unwrap();

        let planned = plan(dir.path(), vec![file("functional/new.md", "c")]).unwrap();
        planned.apply(dir.path()).unwrap();

        assert_eq!(
            planned.removed,
            vec![PathBuf::from("functional/old/README.md")]
        );
        assert_eq!(
            planned.stale,
            vec![PathBuf::from("functional/old/edited.md")]
        );
        assert!(!dir.path().join("functional/old/README.md").exists());
        assert!(dir.path().join("functional/old/edited.md").exists());
        assert!(dir.path().join("functional").is_dir());
    }
}
