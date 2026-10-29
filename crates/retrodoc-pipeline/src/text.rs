//! Reading a source file as text and cutting a text for a prompt: shared by the passes that read files.

use std::path::Path;

use crate::error::PipelineError;

/// The file at `relative` (from `repo_root`) as text, invalid UTF-8 replaced.
pub(crate) fn read_file_lossy(repo_root: &Path, relative: &Path) -> Result<String, PipelineError> {
    let abs = repo_root.join(relative);
    let bytes = std::fs::read(&abs).map_err(|source| PipelineError::Read {
        path: relative.to_path_buf(),
        source,
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// `content` cut to its first `max_chars` characters, with a marker when it was cut.
pub(crate) fn truncate_chars(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }
    let truncated: String = content.chars().take(max_chars).collect();
    format!("{truncated}\n… (truncated)")
}
