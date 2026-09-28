//! Repo walk respecting `.gitignore` (+ additional patterns from
//! `retrodoc.toml`), rough classification of encountered files.

use std::path::{Path, PathBuf};

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;

use crate::error::IngestError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Markdown,
    Source,
    Other,
}

impl FileKind {
    fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("md" | "mdx") => FileKind::Markdown,
            Some(
                "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "kt" | "rb" | "php"
                | "c" | "h" | "cpp" | "hpp" | "cs" | "swift" | "scala" | "sql",
            ) => FileKind::Source,
            _ => FileKind::Other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Path relative to the repo root.
    pub path: PathBuf,
    pub kind: FileKind,
    pub size_bytes: u64,
}

/// Walks `repo_root` respecting `.gitignore`, `.ignore`, and the extra
/// patterns supplied (gitignore syntax, e.g. `"*.lock"`, `"vendor/"`).
pub fn walk_repo(repo_root: &Path, extra_ignore: &[String]) -> Result<Vec<FileEntry>, IngestError> {
    if !repo_root.exists() {
        return Err(IngestError::InvalidRepo(repo_root.to_path_buf()));
    }

    let mut overrides = OverrideBuilder::new(repo_root);
    for pattern in extra_ignore {
        // An "!pattern" override excludes; that's the reverse of raw
        // gitignore syntax, so we prefix it ourselves with "!" to expose
        // plain gitignore syntax on the user config side.
        let negated = format!("!{pattern}");
        overrides
            .add(&negated)
            .map_err(|source| IngestError::InvalidIgnorePattern {
                pattern: pattern.clone(),
                source,
            })?;
    }
    // Always ignore RetroDoc's own internal metadata.
    overrides
        .add("!.retrodoc/")
        .map_err(|source| IngestError::InvalidIgnorePattern {
            pattern: ".retrodoc/".to_string(),
            source,
        })?;
    let overrides = overrides
        .build()
        .map_err(|source| IngestError::InvalidIgnorePattern {
            pattern: "<build>".to_string(),
            source,
        })?;

    let walker = WalkBuilder::new(repo_root)
        .hidden(false) // we want to see .github/, .gitlab-ci.yml etc.; only .git is excluded below
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .overrides(overrides)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();

    let mut entries = Vec::new();
    for result in walker {
        let entry = result?;
        let path = entry.path();
        let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
        if !is_file {
            continue;
        }
        let relative = path.strip_prefix(repo_root).unwrap_or(path).to_path_buf();
        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        entries.push(FileEntry {
            kind: FileKind::from_path(&relative),
            path: relative,
            size_bytes,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn walk_respects_gitignore_and_extra_ignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // `.gitignore` is only applied by the `ignore` crate inside a real
        // git repo (`require_git` defaults to true): initialize one here.
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .unwrap();
        fs::write(root.join(".gitignore"), "ignored_by_git.txt\n").unwrap();
        fs::write(root.join("ignored_by_git.txt"), "x").unwrap();
        fs::write(root.join("ignored_by_config.lock"), "x").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("README.md"), "# hi").unwrap();

        let entries = walk_repo(root, &["*.lock".to_string()]).unwrap();
        let paths: Vec<_> = entries.iter().map(|e| e.path.to_string_lossy().to_string()).collect();

        assert!(paths.contains(&"main.rs".to_string()));
        assert!(paths.contains(&"README.md".to_string()));
        assert!(!paths.contains(&"ignored_by_git.txt".to_string()));
        assert!(!paths.contains(&"ignored_by_config.lock".to_string()));
    }

    #[test]
    fn classifies_file_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("lib.rs"), "").unwrap();
        fs::write(root.join("notes.md"), "").unwrap();
        fs::write(root.join("data.bin"), "").unwrap();

        let entries = walk_repo(root, &[]).unwrap();
        let kind_of = |name: &str| {
            entries
                .iter()
                .find(|e| e.path == Path::new(name))
                .map(|e| e.kind)
        };
        assert_eq!(kind_of("lib.rs"), Some(FileKind::Source));
        assert_eq!(kind_of("notes.md"), Some(FileKind::Markdown));
        assert_eq!(kind_of("data.bin"), Some(FileKind::Other));
    }
}
