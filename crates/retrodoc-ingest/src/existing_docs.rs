//! Raw loading of existing Markdown documents listed by
//! `ingest.existing_docs_paths` in `retrodoc.toml`. Structural parsing
//! (sections, front-matter) is left to the "repo map" step (phase 2); here
//! we just locate the files and their content.

use std::path::{Path, PathBuf};

use crate::error::IngestError;

#[derive(Debug, Clone)]
pub struct ExistingDoc {
    /// Path relative to the repo root.
    pub path: PathBuf,
    pub content: String,
}

/// Resolves `existing_docs_paths` (files or folders, relative to the repo
/// root) into a list of loaded Markdown documents.
///
/// # Errors
///
/// Returns an error if a configured file or folder exists but is unreadable
/// (permissions, invalid encoding).
pub fn load_existing_docs(
    repo_root: &Path,
    existing_docs_paths: &[String],
) -> Result<Vec<ExistingDoc>, IngestError> {
    let mut docs = Vec::new();
    for configured in existing_docs_paths {
        let abs = repo_root.join(configured);
        if !abs.exists() {
            continue;
        }
        if abs.is_dir() {
            collect_markdown_in_dir(repo_root, &abs, &mut docs)?;
        } else if is_markdown(&abs) {
            push_doc(repo_root, &abs, &mut docs)?;
        }
    }
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    docs.dedup_by(|a, b| a.path == b.path);
    Ok(docs)
}

fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("md" | "mdx")
    )
}

fn push_doc(
    repo_root: &Path,
    abs_path: &Path,
    docs: &mut Vec<ExistingDoc>,
) -> Result<(), IngestError> {
    let content = std::fs::read_to_string(abs_path).map_err(|source| IngestError::Read {
        path: abs_path.to_path_buf(),
        source,
    })?;
    let relative = abs_path
        .strip_prefix(repo_root)
        .unwrap_or(abs_path)
        .to_path_buf();
    docs.push(ExistingDoc {
        path: relative,
        content,
    });
    Ok(())
}

fn collect_markdown_in_dir(
    repo_root: &Path,
    dir: &Path,
    docs: &mut Vec<ExistingDoc>,
) -> Result<(), IngestError> {
    for entry in std::fs::read_dir(dir).map_err(|source| IngestError::Read {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| IngestError::Read {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            // Skip RetroDoc's own output folder if it's reused as input.
            if path.file_name().and_then(|n| n.to_str()) == Some("_retrodoc") {
                continue;
            }
            collect_markdown_in_dir(repo_root, &path, docs)?;
        } else if is_markdown(&path) {
            push_doc(repo_root, &path, docs)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_files_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("README.md"), "# root").unwrap();
        fs::create_dir_all(root.join("docs/sub")).unwrap();
        fs::write(root.join("docs/a.md"), "# a").unwrap();
        fs::write(root.join("docs/sub/b.md"), "# b").unwrap();
        fs::write(root.join("docs/ignore.txt"), "not markdown").unwrap();

        let docs =
            load_existing_docs(root, &["README.md".to_string(), "docs".to_string()]).unwrap();
        let paths: Vec<_> = docs
            .iter()
            .map(|d| d.path.to_string_lossy().to_string())
            .collect();

        assert!(paths.contains(&"README.md".to_string()));
        assert!(paths.contains(&"docs/a.md".to_string()));
        assert!(paths.contains(&"docs/sub/b.md".to_string()));
        assert!(!paths.iter().any(|p| p.ends_with("ignore.txt")));
    }

    #[test]
    fn missing_configured_path_is_skipped_silently() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let docs = load_existing_docs(root, &["does-not-exist.md".to_string()]).unwrap();
        assert!(docs.is_empty());
    }
}
