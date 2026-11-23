//! The references of the source files resolved to the files that define them (`specs/resolved-references.md`):
//! what a file imports, inherits from and calls through a receiver the code makes known, with no guess by a bare
//! method name. One resolver per language, behind its grammar feature; a file of any other language gets no
//! resolved reference and keeps the lexical follow of the slices.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[cfg(feature = "python")]
mod python;
#[cfg(feature = "ruby")]
mod ruby;

/// Per file, the other files of the repository it references and how many times.
pub(crate) type Counts = BTreeMap<PathBuf, BTreeMap<PathBuf, usize>>;

/// The resolved references of a set of files.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    references: Counts,
}

impl Resolved {
    /// The files `file` references, most referenced first, then by path; none for a file of a language with no
    /// resolver.
    #[must_use]
    pub fn targets(&self, file: &Path) -> Vec<PathBuf> {
        let Some(targets) = self.references.get(file) else {
            return Vec::new();
        };
        let mut ranked: Vec<(&PathBuf, usize)> = targets.iter().map(|(t, n)| (t, *n)).collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        ranked.into_iter().map(|(t, _)| t.clone()).collect()
    }
}

/// Resolves the references of `files` (paths relative to `repo_root`) among themselves: a target is always one of
/// `files` and never the referencing file. An unreadable file references nothing and defines nothing.
#[must_use]
#[cfg_attr(
    not(any(feature = "python", feature = "ruby")),
    allow(unused_variables)
)]
pub fn resolve(repo_root: &Path, files: &[PathBuf]) -> Resolved {
    #[cfg_attr(not(any(feature = "python", feature = "ruby")), allow(unused_mut))]
    let mut references = Counts::new();
    #[cfg(feature = "python")]
    python::resolve(repo_root, files, &mut references);
    #[cfg(feature = "ruby")]
    ruby::resolve(repo_root, files, &mut references);
    for (file, targets) in &mut references {
        targets.remove(file);
    }
    references.retain(|_, targets| !targets.is_empty());
    Resolved { references }
}

/// The text of `path` under `repo_root`, invalid UTF-8 replaced; `None` when unreadable.
#[cfg_attr(not(any(feature = "python", feature = "ruby")), allow(dead_code))]
fn read(repo_root: &Path, path: &Path) -> Option<String> {
    std::fs::read(repo_root.join(path))
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}
