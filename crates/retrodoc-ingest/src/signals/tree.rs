//! The tree two levels deep: where the project puts things, with no LLM.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::walker::FileEntry;

use super::{Signal, SignalKind};

/// Directory lines kept.
const MAX_LINES: usize = 80;

/// One signal listing the directories down to two levels, each with the
/// number of files below it. `None` for a repo with no file.
#[must_use]
pub fn tree_overview(files: &[FileEntry]) -> Option<Signal> {
    let mut counts: BTreeMap<Vec<String>, usize> = BTreeMap::new();
    for file in files {
        let parts: Vec<String> = file
            .path
            .parent()
            .into_iter()
            .flat_map(|p| p.components())
            .take(2)
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        for depth in 1..=parts.len() {
            *counts.entry(parts[..depth].to_vec()).or_default() += 1;
        }
    }
    if files.is_empty() {
        return None;
    }
    let root_files = files
        .iter()
        .filter(|f| f.path.components().count() == 1)
        .count();
    let mut text = format!("(root): {root_files} files\n");
    for (parts, count) in counts.iter().take(MAX_LINES) {
        let indent = "  ".repeat(parts.len() - 1);
        let _ = writeln!(text, "{indent}{}/: {count} files", parts[parts.len() - 1]);
    }
    if counts.len() > MAX_LINES {
        let _ = writeln!(text, "… {} more directories", counts.len() - MAX_LINES);
    }
    Some(Signal {
        kind: SignalKind::Tree,
        origin: "tree".to_string(),
        text: text.trim_end().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::FileKind;
    use std::path::PathBuf;

    fn file(path: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            kind: FileKind::Source,
            size_bytes: 1,
        }
    }

    #[test]
    fn lists_two_levels_with_file_counts() {
        let signal = tree_overview(&[
            file("README.md"),
            file("app/models/a.rb"),
            file("app/models/b.rb"),
            file("app/models/deep/c.rb"),
            file("app/main.rb"),
            file("db/schema.rb"),
        ])
        .unwrap();
        assert_eq!(signal.kind, SignalKind::Tree);
        assert_eq!(
            signal.text,
            "(root): 1 files\napp/: 4 files\n  models/: 3 files\ndb/: 1 files"
        );
    }

    #[test]
    fn an_empty_repo_has_no_tree() {
        assert!(tree_overview(&[]).is_none());
    }
}
