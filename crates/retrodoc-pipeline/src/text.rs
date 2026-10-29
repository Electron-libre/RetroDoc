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
    match content.char_indices().nth(max_chars) {
        None => content.to_string(),
        Some((cut, _)) => format!("{}\n… (truncated)", &content[..cut]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_of_exactly_max_chars_is_kept_whole() {
        assert_eq!(truncate_chars("héllo", 5), "héllo");
    }

    #[test]
    fn a_text_one_char_over_is_cut_on_a_char_boundary() {
        assert_eq!(truncate_chars("héllo!", 5), "héllo\n… (truncated)");
        assert_eq!(truncate_chars("ééé", 1), "é\n… (truncated)");
    }

    #[test]
    fn a_zero_budget_keeps_only_the_marker() {
        assert_eq!(truncate_chars("abc", 0), "\n… (truncated)");
        assert_eq!(truncate_chars("", 0), "");
    }
}
