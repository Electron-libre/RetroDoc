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

/// The format of a cache written before the version was recorded (CFV-7).
const UNRECORDED: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct FormatFile {
    version: u32,
}

/// A cache this binary must not read.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FormatError {
    #[error(
        "the cache .retrodoc/cache is in format {found}, this RetroDoc reads format {}: run `retrodoc generate --force` to write it again",
        FORMAT_VERSION
    )]
    Older { found: u32 },
    #[error(
        "the cache .retrodoc/cache is in format {found}, written by a later RetroDoc; this one reads format {}: update RetroDoc",
        FORMAT_VERSION
    )]
    Later { found: u32 },
    #[error("{} cannot be read ({reason}): update RetroDoc, or remove the file and run `retrodoc generate --force`", path.display())]
    Unreadable { path: PathBuf, reason: String },
}

/// The format version of the cache under `repo_root`; format 1 when it was not recorded.
///
/// # Errors
///
/// `format.json` exists but cannot be read or parsed.
pub fn saved_version(repo_root: &Path) -> Result<u32, FormatError> {
    let path = Artifact::Format.path(repo_root);
    let unreadable = |reason: String| FormatError::Unreadable {
        path: PathBuf::from(Artifact::Format.relative_path()),
        reason,
    };
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<FormatFile>(&raw)
            .map(|f| f.version)
            .map_err(|e| unreadable(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(UNRECORDED),
        Err(e) => Err(unreadable(e.to_string())),
    }
}

/// Checks that the cache under `repo_root` is in the format this binary reads.
///
/// # Errors
///
/// The cache is of another format, or its `format.json` cannot be read.
pub fn check(repo_root: &Path) -> Result<(), FormatError> {
    let found = saved_version(repo_root)?;
    match found.cmp(&FORMAT_VERSION) {
        std::cmp::Ordering::Less => Err(FormatError::Older { found }),
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Greater => Err(FormatError::Later { found }),
    }
}

/// Records that the cache under `repo_root` is in the format of this binary; written only when it
/// says otherwise, so a rerun rewrites no file.
///
/// # Errors
///
/// The file can't be written.
pub fn record(repo_root: &Path) -> Result<(), PipelineError> {
    let path = Artifact::Format.path(repo_root);
    if path.is_file() && saved_version(repo_root) == Ok(FORMAT_VERSION) {
        return Ok(());
    }
    save_json(
        &path,
        &FormatFile {
            version: FORMAT_VERSION,
        },
    )
}
