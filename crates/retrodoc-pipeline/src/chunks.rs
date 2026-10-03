//! Cutting long files into chunks for the passes that read whole files
//! (entry points, glossary), instead of silently truncating them.

use std::path::Path;

/// A file longer than this many chunks is cut there (a 40 KB controller is
/// already 8 calls): the rest is logged as not read.
const MAX_CHUNKS_PER_FILE: usize = 8;

/// The texts sent for one file by `pass`: its chunks of about `max_chars`,
/// each marked "(part i/n)" when there are several, the end dropped (with a
/// warning) past [`MAX_CHUNKS_PER_FILE`].
pub(crate) fn file_chunks(path: &Path, content: &str, max_chars: usize, pass: &str) -> Vec<String> {
    let mut chunks = split_chunks(content, max_chars);
    if chunks.len() > MAX_CHUNKS_PER_FILE {
        tracing::warn!(
            file = %path.display(),
            chunks = chunks.len(),
            pass,
            "file too long, the end is not read"
        );
        chunks.truncate(MAX_CHUNKS_PER_FILE);
    }
    let total = chunks.len();
    if total > 1 {
        for (i, chunk) in chunks.iter_mut().enumerate() {
            *chunk = format!("(part {}/{total})\n{chunk}", i + 1);
        }
    }
    chunks
}

/// Cuts `content` into chunks of about `max_chars`, on line boundaries and
/// preferably before a blank line (so a method is not split from its `def`).
/// A single line longer than `max_chars` is cut by characters.
pub(crate) fn split_chunks(content: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_len = 0;
    for line in content.split_inclusive('\n') {
        let len = line.chars().count();
        if current_len > 0 && current_len + len > max_chars {
            if let Some(pos) = current
                .rfind("\n\n")
                .filter(|pos| pos + 2 > current.len() / 2)
            {
                let rest = current.split_off(pos + 2);
                chunks.push(std::mem::replace(&mut current, rest));
                current_len = current.chars().count();
            } else {
                chunks.push(std::mem::take(&mut current));
                current_len = 0;
            }
        }
        if len > max_chars {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                current_len = 0;
            }
            let chars: Vec<char> = line.chars().collect();
            chunks.extend(chars.chunks(max_chars).map(|piece| piece.iter().collect()));
            continue;
        }
        current.push_str(line);
        current_len += len;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_content_is_one_chunk_and_long_content_loses_nothing() {
        assert_eq!(split_chunks("def a; end\n", 100), vec!["def a; end\n"]);
        assert!(split_chunks("", 100).is_empty());

        let method = "def action\n  work\nend\n\n";
        let content = method.repeat(10);
        let chunks = split_chunks(&content, 70);
        assert!(chunks.len() > 1);
        assert_eq!(chunks.concat(), content);
        // Cut between methods, never inside one.
        assert!(chunks.iter().all(|c| c.starts_with("def action")));

        let one_line = "y".repeat(25);
        let chunks = split_chunks(&one_line, 10);
        assert_eq!(chunks.concat(), one_line);
        assert_eq!(chunks.len(), 3);
    }
}
