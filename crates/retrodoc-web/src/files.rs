//! What the files page shows: the tree of the source files of the last run,
//! each with the number of features, use cases and rules that cite it. Only
//! paths: the lines of a file are on its page, for a cited file or one the
//! debt lists only.

use std::collections::{BTreeMap, BTreeSet};

use retrodoc_mcp::GeneratedDocs;

use crate::code::citations;
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
    /// A cited file, or one the debt lists: its page.
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

    /// Adds the rows of what it holds under `prefix`; returns its files
    /// cited and all its files.
    fn rows(
        &self,
        prefix: &str,
        depth: usize,
        // The files served though no doc cites them.
        served: &BTreeSet<String>,
        rows: &mut Vec<FileRow>,
    ) -> (usize, usize) {
        let (mut cited, mut total) = (0, 0);
        for (name, dir) in &self.dirs {
            let path = format!("{prefix}{name}/");
            let at = rows.len();
            rows.push(FileRow {
                path: path.clone(),
                name: format!("{name}/"),
                depth,
                dir: true,
                cited: 0,
                total: 0,
                citers: 0,
                href: None,
            });
            let (dir_cited, dir_total) = dir.rows(&path, depth + 1, served, rows);
            rows[at].cited = dir_cited;
            rows[at].total = dir_total;
            cited += dir_cited;
            total += dir_total;
        }
        for (name, citers) in &self.files {
            let path = format!("{prefix}{name}");
            rows.push(FileRow {
                href: (*citers > 0 || served.contains(&path)).then(|| code_href(&path, None, None)),
                path,
                name: name.clone(),
                depth,
                dir: false,
                cited: 0,
                total: 0,
                citers: *citers,
            });
            cited += usize::from(*citers > 0);
            total += 1;
        }
        (cited, total)
    }
}

/// The tree of the files whose hash the last run recorded, of those the docs
/// cite and of those the debt lists: built once, the paths don't change while
/// the server runs.
#[must_use]
pub fn files_page(docs: &GeneratedDocs) -> FilesPage {
    let mut citers = citers_by_file(docs);
    if let Some(freshness) = docs.freshness() {
        for path in freshness.recorded_paths() {
            citers.entry(path.to_string()).or_default();
        }
    }
    let debt: BTreeSet<String> = docs.uncovered().paths().into_iter().collect();
    for path in &debt {
        citers.entry(path.clone()).or_default();
    }
    let mut root = Dir::default();
    for (path, count) in &citers {
        root.insert(path, *count);
    }
    let mut rows = Vec::new();
    let (cited, total) = root.rows("", 0, &debt, &mut rows);
    FilesPage { cited, total, rows }
}

/// For each cited file, the number of features, use cases and rules citing it.
fn citers_by_file(docs: &GeneratedDocs) -> BTreeMap<String, usize> {
    let mut citers = BTreeMap::new();
    for (_, paths) in citations(docs) {
        for path in paths {
            *citers.entry(path).or_default() += 1;
        }
    }
    citers
}
