//! Reading a source file as text and cutting a text for a prompt: shared by the passes that read files.

use std::io::{ErrorKind, Read};
use std::path::{Component, Path, PathBuf};

use crate::error::PipelineError;

/// The file at `relative` (from `repo_root`) as text, invalid UTF-8 replaced. A path that may come from a saved
/// artifact is refused, as unreadable, when it would read a file outside the repository.
pub(crate) fn read_file_lossy(repo_root: &Path, relative: &Path) -> Result<String, PipelineError> {
    let read_error = |source| PipelineError::Read {
        path: relative.to_path_buf(),
        source,
    };
    let real = resolve_inside_repo(repo_root, relative).map_err(read_error)?;
    let bytes = std::fs::read(&real).map_err(read_error)?;
    Ok(decode_lossy(bytes))
}

/// The first `max_bytes` bytes of the file at `relative` as text, cut back to the start of a character and invalid
/// UTF-8 replaced, read as `read_file_lossy` reads: only the prefix is read.
pub(crate) fn read_file_prefix_lossy(
    repo_root: &Path,
    relative: &Path,
    max_bytes: usize,
) -> Result<String, PipelineError> {
    let read_error = |source| PipelineError::Read {
        path: relative.to_path_buf(),
        source,
    };
    let real = resolve_inside_repo(repo_root, relative).map_err(read_error)?;
    let file = std::fs::File::open(&real).map_err(read_error)?;
    // One byte past the budget tells whether the cut falls inside a character.
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() > max_bytes {
        let is_continuation = |byte: u8| byte & 0b1100_0000 == 0b1000_0000;
        // A character has at most three continuation bytes; past them the bytes are invalid anyway.
        let back = (1..=3)
            .take_while(|&back| back <= max_bytes && is_continuation(bytes[max_bytes + 1 - back]))
            .count();
        bytes.truncate(max_bytes - back);
    }
    Ok(decode_lossy(bytes))
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
