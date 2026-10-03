//! Cutting long files into chunks for the passes that read whole files
//! (entry points, glossary), instead of silently truncating them.
//!
//! Where to cut depends on the language, so it is not hard-coded: the role
//! identification pass (`roles.rs`) asks the LLM, along with the stack, for
//! one regex per language matching the line that *starts* a module, class or
//! function ([`ChunkBoundary`]), saved in the hand-editable `roles.yaml`. The
//! chunker cuts before the last such line in the second half of a chunk, and
//! falls back to a blank line, then to any line, when there is none (no rule
//! for the extension, invalid regex, dense code).

use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// A file longer than this many chunks is cut there (a 40 KB controller is
/// already 8 calls): the rest is logged as not read.
const MAX_CHUNKS_PER_FILE: usize = 8;

/// Where the units of a language start, as identified by the LLM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkBoundary {
    /// File extensions the rule applies to, with or without the dot.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Regex matching a line that starts a module, class or function/method
    /// definition (never the line that ends it).
    pub pattern: String,
}

/// The compiled [`ChunkBoundary`] rules of a repo.
#[derive(Debug, Default)]
pub(crate) struct Splitter {
    rules: Vec<(Vec<String>, Regex)>,
}

impl Splitter {
    /// A rule whose regex doesn't compile is skipped with a warning.
    pub(crate) fn new(boundaries: &[ChunkBoundary]) -> Self {
        let rules = boundaries
            .iter()
            .filter_map(|boundary| match Regex::new(&boundary.pattern) {
                Ok(regex) => {
                    let extensions = boundary
                        .extensions
                        .iter()
                        .map(|e| e.trim_start_matches('.').to_lowercase())
                        .collect();
                    Some((extensions, regex))
                }
                Err(err) => {
                    tracing::warn!(
                        pattern = %boundary.pattern,
                        error = %err,
                        "invalid chunk boundary pattern, ignored"
                    );
                    None
                }
            })
            .collect();
        Splitter { rules }
    }

    fn boundary_for(&self, path: &Path) -> Option<&Regex> {
        let extension = path.extension()?.to_string_lossy().to_lowercase();
        self.rules
            .iter()
            .find(|(extensions, _)| extensions.contains(&extension))
            .map(|(_, regex)| regex)
    }

    /// The texts sent for one file by `pass`: its chunks of about
    /// `max_chars`, each marked "(part i/n)" when there are several, the end
    /// dropped (with a warning) past [`MAX_CHUNKS_PER_FILE`].
    pub(crate) fn file_chunks(
        &self,
        path: &Path,
        content: &str,
        max_chars: usize,
        pass: &str,
    ) -> Vec<String> {
        let mut chunks = split_chunks(content, max_chars, self.boundary_for(path));
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
}

/// Cuts `content` into chunks of about `max_chars`, on line boundaries. When
/// a chunk is full it is cut, if possible in its second half, before the last
/// line matching `boundary` (the start of a unit), else after the last blank
/// line. A single line longer than `max_chars` is cut by characters.
pub(crate) fn split_chunks(
    content: &str,
    max_chars: usize,
    boundary: Option<&Regex>,
) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_len = 0;
    // Byte offsets in `current` where a cut is allowed.
    let mut unit_cut: Option<usize> = None;
    let mut blank_cut: Option<usize> = None;
    for line in content.split_inclusive('\n') {
        let len = line.chars().count();
        if current_len > 0 && current_len + len > max_chars {
            let half = current.len() / 2;
            let cut = unit_cut
                .filter(|cut| *cut > half)
                .or_else(|| blank_cut.filter(|cut| *cut > half));
            if let Some(cut) = cut {
                let rest = current.split_off(cut);
                chunks.push(std::mem::replace(&mut current, rest));
                current_len = current.chars().count();
            } else {
                chunks.push(std::mem::take(&mut current));
                current_len = 0;
            }
            unit_cut = None;
            blank_cut = None;
        }
        if len > max_chars {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                current_len = 0;
                unit_cut = None;
                blank_cut = None;
            }
            let chars: Vec<char> = line.chars().collect();
            chunks.extend(chars.chunks(max_chars).map(|piece| piece.iter().collect()));
            continue;
        }
        if !current.is_empty() && boundary.is_some_and(|b| b.is_match(line)) {
            unit_cut = Some(current.len());
        }
        current.push_str(line);
        current_len += len;
        if line.trim().is_empty() {
            blank_cut = Some(current.len());
        }
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
        assert_eq!(
            split_chunks("def a; end\n", 100, None),
            vec!["def a; end\n"]
        );
        assert!(split_chunks("", 100, None).is_empty());

        let one_line = "y".repeat(25);
        let chunks = split_chunks(&one_line, 10, None);
        assert_eq!(chunks.concat(), one_line);
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn without_a_boundary_rule_it_cuts_after_a_blank_line() {
        let content = "alpha\nbeta\n\n".repeat(10);
        let chunks = split_chunks(&content, 40, None);
        assert!(chunks.len() > 1);
        assert_eq!(chunks.concat(), content);
        assert!(chunks.iter().all(|c| c.starts_with("alpha")));
    }

    #[test]
    fn a_boundary_rule_cuts_before_the_unit_even_without_blank_lines() {
        // Brace language, no blank line between functions.
        let content = "func a() {\n  work()\n}\n".repeat(10);
        let boundary = Regex::new(r"^func ").unwrap();
        let chunks = split_chunks(&content, 70, Some(&boundary));
        assert!(chunks.len() > 1);
        assert_eq!(chunks.concat(), content);
        assert!(chunks.iter().all(|c| c.starts_with("func a")));
    }

    #[test]
    fn the_rule_is_picked_by_extension_and_a_bad_regex_is_ignored() {
        let splitter = Splitter::new(&[
            ChunkBoundary {
                extensions: vec!["(".to_string()],
                pattern: "(".to_string(),
            },
            ChunkBoundary {
                extensions: vec![".GO".to_string()],
                pattern: "^func ".to_string(),
            },
        ]);
        assert!(splitter.boundary_for(Path::new("a/main.go")).is_some());
        assert!(splitter.boundary_for(Path::new("a/main.rb")).is_none());
        assert!(splitter.boundary_for(Path::new("Makefile")).is_none());
    }

    #[test]
    fn long_files_are_marked_with_their_parts_and_capped() {
        let splitter = Splitter::default();
        let content = "x\n".repeat(10 * 5);
        let chunks = splitter.file_chunks(Path::new("a.rb"), &content, 10, "test");
        assert_eq!(chunks.len(), MAX_CHUNKS_PER_FILE);
        assert!(chunks[0].starts_with("(part 1/8)"));
    }
}
