//! Reading a source file as text and cutting a text for a prompt: shared by the passes that read files.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Component, Path, PathBuf};

use crate::error::PipelineError;

/// The file at `relative` (from `repo_root`) as text, invalid UTF-8 replaced. A path that may come from a saved
/// artifact is refused, as unreadable, when it would read a file outside the repository.
pub(crate) fn read_file_lossy(repo_root: &Path, relative: &Path) -> Result<String, PipelineError> {
    read_inside_repo(repo_root, relative, |mut file, bytes| {
        file.read_to_end(bytes).map(drop)
    })
}

/// The first `max_bytes` bytes of the file at `relative` as text, a character the cut splits left out and invalid
/// UTF-8 replaced, read as `read_file_lossy` reads: only the prefix is read.
pub(crate) fn read_file_prefix_lossy(
    repo_root: &Path,
    relative: &Path,
    max_bytes: usize,
) -> Result<String, PipelineError> {
    read_inside_repo(repo_root, relative, |file, bytes| {
        // One byte past the budget tells whether the file goes on after the cut.
        let limit = u64::try_from(max_bytes).map_or(u64::MAX, |max| max.saturating_add(1));
        file.take(limit).read_to_end(bytes)?;
        if bytes.len() > max_bytes {
            bytes.truncate(max_bytes);
            // A character split by the cut ends the prefix with the start of a valid sequence.
            let split = bytes.utf8_chunks().last().map_or(0, |chunk| {
                let tail = chunk.invalid();
                if std::str::from_utf8(tail).is_err_and(|err| err.error_len().is_none()) {
                    tail.len()
                } else {
                    0
                }
            });
            bytes.truncate(max_bytes - split);
        }
        Ok(())
    })
}

/// The bytes `read_bytes` takes from the file at `relative`, resolved by `resolve_inside_repo`, decoded lossily.
fn read_inside_repo(
    repo_root: &Path,
    relative: &Path,
    read_bytes: impl FnOnce(File, &mut Vec<u8>) -> std::io::Result<()>,
) -> Result<String, PipelineError> {
    let read_error = |source| PipelineError::Read {
        path: relative.to_path_buf(),
        source,
    };
    let real = resolve_inside_repo(repo_root, relative).map_err(read_error)?;
    let file = File::open(&real).map_err(read_error)?;
    let mut bytes = Vec::new();
    read_bytes(file, &mut bytes).map_err(read_error)?;
    Ok(decode_lossy(bytes))
}

/// `text` without its last `n` characters, empty when it has no more: one walk from the end.
pub(crate) fn without_last_chars(text: &str, n: usize) -> &str {
    match n.checked_sub(1) {
        None => text,
        Some(back) => text
            .char_indices()
            .nth_back(back)
            .map_or("", |(cut, _)| &text[..cut]),
    }
}

/// `bytes` as text, invalid UTF-8 replaced: valid UTF-8 keeps the buffer read, only invalid bytes cost a copy.
fn decode_lossy(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

/// `relative` resolved under `repo_root`, refused when it is absolute, has a `..` component or resolves outside the
/// repository through a symbolic link. A link that stays inside the repository is followed.
fn resolve_inside_repo(repo_root: &Path, relative: &Path) -> std::io::Result<PathBuf> {
    let outside = || std::io::Error::new(ErrorKind::InvalidInput, "the path leaves the repository");
    let plain = relative
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
    if !plain {
        return Err(outside());
    }
    let real = repo_root.join(relative).canonicalize()?;
    if real.starts_with(repo_root.canonicalize()?) {
        Ok(real)
    } else {
        Err(outside())
    }
}

/// The first `max_chars` characters of `text` when it has more, else `None`: one walk of at most `max_chars`
/// characters, whatever the length of `text`.
pub(crate) fn char_prefix(text: &str, max_chars: usize) -> Option<&str> {
    text.char_indices()
        .nth(max_chars)
        .map(|(cut, _)| &text[..cut])
}

/// The first `max_chars` characters of `text`, all of it when it is shorter.
pub(crate) fn first_chars(text: &str, max_chars: usize) -> &str {
    char_prefix(text, max_chars).unwrap_or(text)
}

/// `content` cut to its first `max_chars` characters, with a marker when it was cut.
pub(crate) fn truncate_chars(content: &str, max_chars: usize) -> String {
    match char_prefix(content, max_chars) {
        None => content.to_string(),
        Some(head) => format!("{head}\n… (truncated)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_read_as_text_with_invalid_utf8_replaced() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ok.rb"), "héllo").unwrap();
        std::fs::write(dir.path().join("bad.rb"), b"caf\xe9").unwrap();
        assert_eq!(
            read_file_lossy(dir.path(), Path::new("ok.rb")).unwrap(),
            "héllo"
        );
        assert_eq!(
            read_file_lossy(dir.path(), Path::new("bad.rb")).unwrap(),
            "caf\u{fffd}"
        );
        assert!(read_file_lossy(dir.path(), Path::new("missing.rb")).is_err());
    }

    /// A repo holding `a.rb`, next to a directory outside it holding `secret.rb`.
    fn repo_and_outside() -> (tempfile::TempDir, tempfile::TempDir) {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(repo.path().join("a.rb"), "inside").unwrap();
        std::fs::write(outside.path().join("secret.rb"), "secret").unwrap();
        (repo, outside)
    }

    #[test]
    fn an_absolute_path_is_refused_even_inside_the_repo() {
        let (repo, outside) = repo_and_outside();
        assert!(read_file_lossy(repo.path(), &outside.path().join("secret.rb")).is_err());
        assert!(read_file_lossy(repo.path(), &repo.path().join("a.rb")).is_err());
    }

    #[test]
    fn a_path_climbing_out_with_dot_dot_is_refused() {
        let (repo, outside) = repo_and_outside();
        let name = outside.path().file_name().unwrap();
        let climbing = Path::new("..").join(name).join("secret.rb");
        assert!(read_file_lossy(repo.path(), &climbing).is_err());
        std::fs::create_dir(repo.path().join("lib")).unwrap();
        assert!(read_file_lossy(repo.path(), Path::new("lib/../a.rb")).is_err());
    }

    #[test]
    fn a_dot_slash_path_is_read() {
        let (repo, _outside) = repo_and_outside();
        assert_eq!(
            read_file_lossy(repo.path(), Path::new("./a.rb")).unwrap(),
            "inside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_resolving_outside_the_repo_is_refused() {
        use std::os::unix::fs::symlink;
        let (repo, outside) = repo_and_outside();
        symlink(
            outside.path().join("secret.rb"),
            repo.path().join("file_link.rb"),
        )
        .unwrap();
        symlink(outside.path(), repo.path().join("dir_link")).unwrap();
        assert!(read_file_lossy(repo.path(), Path::new("file_link.rb")).is_err());
        assert!(read_file_lossy(repo.path(), Path::new("dir_link/secret.rb")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_resolving_inside_the_repo_is_read() {
        use std::os::unix::fs::symlink;
        let (repo, _outside) = repo_and_outside();
        symlink(repo.path().join("a.rb"), repo.path().join("alias.rb")).unwrap();
        assert_eq!(
            read_file_lossy(repo.path(), Path::new("alias.rb")).unwrap(),
            "inside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_repo_root_reached_through_a_link_still_reads_its_files() {
        use std::os::unix::fs::symlink;
        let (repo, outside) = repo_and_outside();
        let root_link = outside.path().join("root_link");
        symlink(repo.path(), &root_link).unwrap();
        assert_eq!(
            read_file_lossy(&root_link, Path::new("a.rb")).unwrap(),
            "inside"
        );
    }

    #[test]
    fn a_prefix_read_cut_inside_a_character_drops_the_character() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rb"), "abé").unwrap();
        let read = |max| read_file_prefix_lossy(dir.path(), Path::new("a.rb"), max).unwrap();
        assert_eq!(read(3), "ab");
        assert_eq!(read(4), "abé");
        assert_eq!(read(10), "abé");
        assert_eq!(read(0), "");
    }

    #[test]
    fn a_prefix_read_cut_before_a_stray_continuation_byte_keeps_the_character_before() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("latin1.rb"), b"abc\xa9\xa9").unwrap();
        assert_eq!(
            read_file_prefix_lossy(dir.path(), Path::new("latin1.rb"), 3).unwrap(),
            "abc"
        );
        assert_eq!(
            read_file_prefix_lossy(dir.path(), Path::new("latin1.rb"), 4).unwrap(),
            "abc\u{fffd}"
        );
    }

    #[test]
    fn a_text_loses_its_last_characters() {
        assert_eq!(without_last_chars("héllo", 2), "hél");
        assert_eq!(without_last_chars("hé", 2), "");
        assert_eq!(without_last_chars("h", 2), "");
        assert_eq!(without_last_chars("héllo", 0), "héllo");
    }

    #[test]
    fn a_prefix_read_keeps_invalid_bytes_replaced() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bad.rb"), b"caf\xe9 ok").unwrap();
        assert_eq!(
            read_file_prefix_lossy(dir.path(), Path::new("bad.rb"), 6).unwrap(),
            "caf\u{fffd} o"
        );
    }

    #[test]
    fn a_prefix_read_outside_the_repo_is_refused() {
        let (repo, outside) = repo_and_outside();
        assert!(
            read_file_prefix_lossy(repo.path(), &outside.path().join("secret.rb"), 10).is_err()
        );
        assert!(read_file_prefix_lossy(repo.path(), Path::new("missing.rb"), 10).is_err());
    }

    #[test]
    fn a_prefix_is_given_only_when_the_text_is_longer() {
        assert_eq!(char_prefix("héllo", 5), None);
        assert_eq!(char_prefix("héllo!", 5), Some("héllo"));
        assert_eq!(char_prefix("ééé", 1), Some("é"));
        assert_eq!(char_prefix("abc", 0), Some(""));
        assert_eq!(char_prefix("", 0), None);
    }

    #[test]
    fn first_chars_keeps_a_shorter_text_whole() {
        assert_eq!(first_chars("héllo", 5), "héllo");
        assert_eq!(first_chars("héllo!", 2), "hé");
        assert_eq!(first_chars("", 3), "");
    }

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
