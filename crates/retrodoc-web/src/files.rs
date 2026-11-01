//! What the files page shows: the tree of the source files of the last run,
//! each with the number of features, use cases and rules that cite it. Only
//! paths: the lines of a file are on its page, for a cited file only.

use std::collections::{BTreeMap, BTreeSet};

use retrodoc_core::model::RuleEvidence;
use retrodoc_mcp::GeneratedDocs;

use crate::view::code_href;

/// The files page.
pub struct FilesPage {
    pub cited: usize,
    pub total: usize,
    /// Depth first, each directory before what it holds, the subdirectories
    /// before the files.
    pub rows: Vec<FileRow>,
}

/// A directory or a file of the tree.
pub struct FileRow {
    /// From the repository root, a directory ending with `/`.
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub dir: bool,
    /// A directory: its files cited, out of `total`.
    pub cited: usize,
    pub total: usize,
    /// A file: the features, use cases and rules that cite it.
    pub citers: usize,
    /// A cited file: its page.
    pub href: Option<String>,
}

#[derive(Default)]
struct Dir {
    dirs: BTreeMap<String, Dir>,
    /// Each file and the number of items that cite it.
    files: BTreeMap<String, usize>,
}

impl Dir {
    fn insert(&mut self, path: &str, citers: usize) {
        match path.split_once('/') {
            Some((dir, rest)) => self
                .dirs
                .entry(dir.to_string())
                .or_default()
                .insert(rest, citers),
            None => {
                self.files.insert(path.to_string(), citers);
            }
        }
    }

    /// Its files cited, and all its files.
    fn counts(&self) -> (usize, usize) {
        self.dirs.values().map(Dir::counts).fold(
            (
                self.files.values().filter(|n| **n > 0).count(),
                self.files.len(),
            ),
            |(cited, total), (c, t)| (cited + c, total + t),
        )
    }

    fn rows(&self, prefix: &str, depth: usize, rows: &mut Vec<FileRow>) {
        for (name, dir) in &self.dirs {
            let path = format!("{prefix}{name}/");
            let (cited, total) = dir.counts();
            rows.push(FileRow {
                path: path.clone(),
                name: format!("{name}/"),
                depth,
                dir: true,
                cited,
                total,
                citers: 0,
                href: None,
            });
            dir.rows(&path, depth + 1, rows);
        }
        for (name, citers) in &self.files {
            let path = format!("{prefix}{name}");
            rows.push(FileRow {
                href: (*citers > 0).then(|| code_href(&path, None, None)),
                path,
                name: name.clone(),
                depth,
                dir: false,
                cited: 0,
                total: 0,
                citers: *citers,
            });
        }
    }
}

/// The tree of the files whose hash the last run recorded and of those the
/// docs cite.
#[must_use]
pub fn files_page(docs: &GeneratedDocs) -> FilesPage {
    let mut citers = citers_by_file(docs);
    if let Some(freshness) = docs.freshness() {
        for path in freshness.recorded_paths() {
            citers.entry(path.to_string()).or_default();
        }
    }
    let mut root = Dir::default();
    for (path, count) in &citers {
        root.insert(path, *count);
    }
    let (cited, total) = root.counts();
    let mut rows = Vec::new();
    root.rows("", 0, &mut rows);
    FilesPage { cited, total, rows }
}

/// For each cited file, the number of features, use cases and rules citing it.
fn citers_by_file(docs: &GeneratedDocs) -> BTreeMap<String, usize> {
    let plain = |path: &str| path.strip_prefix("./").unwrap_or(path).to_string();
    let features = docs.features().iter().map(|f| {
        f.source_paths
            .iter()
            .map(|p| plain(p))
            .collect::<BTreeSet<_>>()
    });
    let use_cases = docs.use_cases().iter().map(|u| {
        u.steps
            .iter()
            .flat_map(|s| &s.source_refs)
            .map(|r| plain(&r.path))
            .collect::<BTreeSet<_>>()
    });
    let rules = docs.rules().iter().map(|r| {
        r.evidence
            .iter()
            .filter_map(|e| match e {
                RuleEvidence::Code { source } => Some(plain(&source.path)),
                _ => None,
            })
            .collect::<BTreeSet<_>>()
    });
    let mut citers = BTreeMap::new();
    for path in features.chain(use_cases).chain(rules).flatten() {
        *citers.entry(path).or_default() += 1;
    }
    citers
}
