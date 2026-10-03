//! Reading and writing the artifacts saved under `.retrodoc/cache/`: one
//! place for the "create the folder, serialize, write" sequence and for the
//! "missing or unreadable is a first run, not an error" rule.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::PipelineError;

/// Missing or unreadable (or no longer matching the type): `None`.
pub(crate) fn load_yaml<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_yaml::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Missing or unreadable (or no longer matching the type): `None`.
pub(crate) fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub(crate) fn save_yaml<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), PipelineError> {
    write(path, &serde_yaml::to_string(value)?)
}

pub(crate) fn save_json<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), PipelineError> {
    write(path, &serde_json::to_string_pretty(value)?)
}

/// For saves made while another error is already being propagated (partial
/// results of an interrupted pass): a failed write must not mask that error,
/// but it must not vanish either.
pub(crate) fn warn_on_error(result: Result<(), PipelineError>) {
    if let Err(error) = result {
        tracing::warn!("could not save an intermediate artifact: {error}");
    }
}

fn write(path: &Path, raw: &str) -> Result<(), PipelineError> {
    let io_error = |source| PipelineError::ArtifactIo {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_error)?;
    }
    std::fs::write(path, raw).map_err(io_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_create_the_folder_and_loads_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("a/b/x.yaml");
        let json = dir.path().join("c/x.json");
        save_yaml(&yaml, &vec!["one".to_string()]).unwrap();
        save_json(&json, &vec![1, 2]).unwrap();
        assert_eq!(load_yaml::<Vec<String>>(&yaml), Some(vec!["one".into()]));
        assert_eq!(load_json::<Vec<i32>>(&json), Some(vec![1, 2]));
    }

    #[test]
    fn missing_or_mismatching_files_load_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.yaml");
        assert_eq!(load_yaml::<Vec<String>>(&path), None);
        std::fs::write(&path, "not: a list").unwrap();
        assert_eq!(load_yaml::<Vec<String>>(&path), None);
    }
}
