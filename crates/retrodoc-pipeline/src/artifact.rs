//! The artifacts of `.retrodoc/cache/` as the passes save them: the layout of `retrodoc_core::cache`
//! with the errors of the pipeline, and what a pass keeps of the saved entries.

use std::path::Path;

use serde::Serialize;

pub use retrodoc_core::cache::{load_json, load_yaml, Artifact};

use crate::error::PipelineError;

/// What a reading pass (glossary, entry points) does with the saved entries of the files it is not
/// given this time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    /// Only the files read now stay: the inventory follows the files it is asked about.
    OnlyRead,
    /// The saved entries of the other files that still exist stay: a repository read in several
    /// runs, a slice at a time.
    Others,
}

/// Writes `value` as YAML at `path`, creating its directory.
///
/// # Errors
///
/// The value can't be serialized or the file can't be written.
pub fn save_yaml<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), PipelineError> {
    Ok(retrodoc_core::cache::save_yaml(path, value)?)
}

pub(crate) fn save_json<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), PipelineError> {
    Ok(retrodoc_core::cache::save_json(path, value)?)
}

/// For saves made while another error is already being propagated (partial
/// results of an interrupted pass): a failed write must not mask that error,
/// but it must not vanish either.
pub(crate) fn warn_on_error(result: Result<(), PipelineError>) {
    if let Err(error) = result {
        tracing::warn!("could not save an intermediate artifact: {error}");
    }
}
