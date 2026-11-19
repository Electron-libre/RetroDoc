//! The format version of the artifacts of `.retrodoc/cache/` (ADR 0050, `specs/cache-format.md`):
//! one integer in `format.json`, written by `generate` before its first artifact and checked by the
//! readers of the generated docs, so a cache another version wrote is refused with the
//! command to run rather than read as empty fields. It is not part of the fingerprints.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::artifact::{save_json, Artifact};
use crate::error::PipelineError;

/// The format of the artifacts this binary writes and reads. Bump it on a change of their shape
/// that an older reader would read wrong, and refresh the frozen sample of the round-trip test.
pub const FORMAT_VERSION: u32 = 1;

/// The format of a cache that holds artifacts and no `format.json`: written before the version was
/// recorded (CFV-7).
const UNRECORDED: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct FormatFile {
    version: u32,
}

/// A cache this binary must not read.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FormatError {
    #[error(
        "the cache .retrodoc/cache is in format {found}, this RetroDoc reads format {reads}: run `retrodoc generate --force` to write it again"
    )]
    Older { found: u32, reads: u32 },
    #[error(
        "the cache .retrodoc/cache is in format {found}, written by a later RetroDoc; this one reads format {reads}: update RetroDoc"
    )]
    Later { found: u32, reads: u32 },
    #[error("{} cannot be read ({reason}): update RetroDoc, or remove the file and run `retrodoc generate --force`", path.display())]
    Unreadable { path: PathBuf, reason: String },
}

/// The version `format.json` holds, `None` when there is no such file.
fn recorded(repo_root: &Path) -> Result<Option<u32>, FormatError> {
    let unreadable = |reason: String| FormatError::Unreadable {
        path: PathBuf::from(Artifact::Format.relative_path()),
        reason,
    };
    match std::fs::read_to_string(Artifact::Format.path(repo_root)) {
        Ok(raw) => serde_json::from_str::<FormatFile>(&raw)
            .map(|f| Some(f.version))
            .map_err(|e| unreadable(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(unreadable(e.to_string())),
    }
}

/// Checks that the cache under `repo_root` is in the format this binary reads; a cache with no
/// artifact is in any format.
///
/// # Errors
///
/// The cache is of another format, or its `format.json` cannot be read.
pub fn check(repo_root: &Path) -> Result<(), FormatError> {
    check_against(repo_root, FORMAT_VERSION)
}

fn check_against(repo_root: &Path, reads: u32) -> Result<(), FormatError> {
    let found = match recorded(repo_root)? {
        Some(found) => found,
        None if holds_artifacts(repo_root) => UNRECORDED,
        None => return Ok(()),
    };
    match found.cmp(&reads) {
        std::cmp::Ordering::Less => Err(FormatError::Older { found, reads }),
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Greater => Err(FormatError::Later { found, reads }),
    }
}

fn holds_artifacts(repo_root: &Path) -> bool {
    Artifact::ALL
        .iter()
        .any(|a| *a != Artifact::Format && a.path(repo_root).exists())
}

/// Records that the cache under `repo_root` is in the format of this binary; written only when it
/// says otherwise, so a rerun rewrites no file.
///
/// # Errors
///
/// The file can't be written.
pub fn record(repo_root: &Path) -> Result<(), PipelineError> {
    if matches!(recorded(repo_root), Ok(Some(FORMAT_VERSION))) {
        return Ok(());
    }
    save_json(
        &Artifact::Format.path(repo_root),
        &FormatFile {
            version: FORMAT_VERSION,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_artifact(dir: &Path) {
        crate::artifact::save_yaml(&Artifact::Features.path(dir), &Vec::<String>::new()).unwrap();
    }

    // spec: CFV-7 (internal: the binary reads one format, a later one is needed to see an older cache)
    #[test]
    fn a_cache_with_artifacts_and_no_format_file_is_format_1_to_a_later_reader() {
        let dir = tempfile::tempdir().unwrap();
        with_artifact(dir.path());

        assert_eq!(
            check_against(dir.path(), 2),
            Err(FormatError::Older { found: 1, reads: 2 })
        );
    }

    // spec: CFV-7 (internal: the binary reads one format, a later one is needed to see an older cache)
    #[test]
    fn a_cache_with_no_artifact_is_read_by_any_version() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(check_against(dir.path(), 2), Ok(()));
        crate::artifact::save_json(&Artifact::Usage.path(dir.path()), &Vec::<u8>::new()).unwrap();
        assert_eq!(
            check_against(dir.path(), 2),
            Err(FormatError::Older { found: 1, reads: 2 })
        );
    }
}
