//! Cutting long files into chunks for the passes that read whole files
//! (entry points, glossary), instead of silently truncating them.
//!
//! The use cases and confidence passes send the code to the LLM as a numbered
//! excerpt: for a long file it is no longer its first characters but the
//! chunks that matter ([`Splitter::excerpt`]): the first chunk (the header),
//! then the ones naming what the feature is about or holding the lines a step
//! cites ([`Focus`]), with absolute line numbers and the omitted ranges marked.
//!
//! Where to cut depends on the language, so it is not hard-coded: the role
//! identification pass (`roles/`) asks the LLM, along with the stack, for
//! one regex per language matching the line that *starts* a module, class or
//! function ([`ChunkBoundary`]), saved in the hand-editable `roles.yaml`. The
//! chunker cuts before the last such line in the second half of a chunk, and
//! falls back to a blank line, then to any line, when there is none (no rule
//! for the extension, invalid regex, dense code).

use std::fmt::Write as _;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::use_cases::numbered_excerpt;

/// A file longer than this many chunks is cut there (a 40 KB controller is
/// already 8 calls): the rest is logged as not read.
const MAX_CHUNKS_PER_FILE: usize = 8;

/// What the LLM is told about the regex dialect of [`ChunkBoundary::pattern`]
/// (models happily write lookaheads, which the `regex` crate rejects).
pub(crate) const REGEX_SYNTAX_HELP: &str = "Regex syntax: the Rust `regex` crate (RE2-like). \
Supported: `^`, `$`, `.`, `\\s`, `\\w`, `\\d`, `\\b`, character classes `[...]`, groups `(...)` and \
non-capturing groups `(?:...)`, alternation `|`, quantifiers `*`, `+`, `?`, `{n,m}` (and lazy `*?`). \
NOT supported, the pattern would be rejected: lookahead and lookbehind (`(?=`, `(?!`, `(?<=`, \
`(?<!`), backreferences (`\\1`), atomic groups and possessive quantifiers. The pattern is matched \
against ONE line at a time (it never contains a newline), so anchor it with `^`. It is written \
inside a JSON string, so every backslash must be doubled (`\\\\s` for `\\s`).";

/// Size of the chunks an excerpt is assembled from: small enough to pick one
/// method out of a large controller.
const EXCERPT_CHUNK_CHARS: usize = 1_200;

/// What an excerpt of a long file should favour (nothing: its beginning).
#[derive(Debug, Default)]
pub(crate) struct Focus {
    /// Identifiers likely to name the code of interest (entry point and
    /// action names): a strong signal. Lowercase.
    pub terms: Vec<String>,
    /// Plain words of the feature's text: a weak signal. Lowercase.
    pub words: Vec<String>,
    /// 1-based inclusive line ranges the code is known to be cited at.
    pub lines: Vec<(u32, u32)>,
}

impl Focus {
    /// Score of a chunk spanning lines `first..=last`: holding a cited line
    /// outweighs any number of matched names, a name outweighs a word.
    fn score(&self, text: &str, first: u32, last: u32) -> u32 {
        let lower = text.to_lowercase();
        let cited = self.lines.iter().any(|(a, b)| *a <= last && *b >= first);
        let count = |list: &[String]| {
            u32::try_from(list.iter().filter(|t| lower.contains(t.as_str())).count())
                .unwrap_or(u32::MAX)
        };
        u32::from(cited) * 100_000 + count(&self.terms) * 100 + count(&self.words)
    }
}

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

/// One chunk of a file rendered for an excerpt.
struct Piece {
    text: String,
    first: u32,
    last: u32,
    score: u32,
}

impl Splitter {
    /// `content` with numbered lines, within about `max_chars`. A file that
    /// fits is given whole; otherwise the first chunk and the best scoring
    /// ones for `focus` (best first, while the budget allows), in file order,
    /// with a marker for each omitted range of lines.
    pub(crate) fn excerpt(
        &self,
        path: &Path,
        content: &str,
        max_chars: usize,
        focus: &Focus,
    ) -> String {
        let whole = numbered_excerpt(content, usize::MAX);
        if whole.len() <= max_chars {
            return whole;
        }
        let pieces = self.pieces(path, content, focus);
        let total_lines = pieces.last().map_or(0, |p| p.last);

        let mut order: Vec<usize> = (1..pieces.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(pieces[i].score), i));
        order.insert(0, 0);
        let mut chosen = vec![false; pieces.len()];
        let mut used = 0;
        for i in order {
            if used + pieces[i].text.len() <= max_chars {
                chosen[i] = true;
                used += pieces[i].text.len();
            }
        }

        let mut out = String::new();
        let mut shown_to = 0;
        for (piece, _) in pieces.iter().zip(&chosen).filter(|(_, c)| **c) {
            if piece.first > shown_to + 1 {
                omitted(&mut out, shown_to + 1, piece.first - 1);
            }
            out.push_str(&piece.text);
            shown_to = piece.last;
        }
        if out.is_empty() {
            out = whole.chars().take(max_chars).collect();
        } else if shown_to < total_lines {
            omitted(&mut out, shown_to + 1, total_lines);
        }
        out
    }

    /// The chunks of `content`, numbered with absolute line numbers.
    fn pieces(&self, path: &Path, content: &str, focus: &Focus) -> Vec<Piece> {
        let mut pieces: Vec<Piece> = Vec::new();
        let mut next_line = 1_u32;
        let mut continues = false;
        for chunk in split_chunks(content, EXCERPT_CHUNK_CHARS, self.boundary_for(path)) {
            let first = next_line;
            let mut text = String::new();
            for (i, line) in chunk.split_inclusive('\n').enumerate() {
                let line = line.trim_end_matches(['\r', '\n']);
                if i == 0 && continues {
                    // The rest of a very long line cut by the chunker.
                    let _ = writeln!(text, "     | {line}");
                } else {
                    let _ = writeln!(text, "{next_line:>4} | {line}");
                    next_line += 1;
                }
            }
            continues = !chunk.ends_with('\n');
            let last = next_line - 1;
            let score = focus.score(&chunk, first, last);
            pieces.push(Piece {
                text,
                first,
                last,
                score,
            });
        }
        pieces
    }
}

fn omitted(out: &mut String, from: u32, to: u32) {
    let _ = writeln!(out, "     … (lines {from}-{to} omitted)");
}

/// `path` without the "(part i/n)" marker a model sometimes copies after the
/// file name it was shown (`config/routes.rb (part 1/2)`).
pub(crate) fn strip_part_marker(path: &str) -> &str {
    let path = path.trim();
    match path.rsplit_once('(') {
        Some((name, marker)) if marker.starts_with("part ") && marker.trim_end().ends_with(')') => {
            name.trim_end()
        }
        _ => path,
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
        assert_eq!(split_chunks("", 100, None), Vec::<String>::new());

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

    /// 40 methods of 11 lines; `target_a` is the 30th (line 321), `target_b`
    /// the 12th (line 123).
    fn controller() -> String {
        let mut content = String::from("class C\n");
        for i in 1..=40 {
            let name = match i {
                30 => "target_a".to_string(),
                12 => "target_b".to_string(),
                _ => format!("action_{i}"),
            };
            let _ = writeln!(content, "  def {name}");
            for step in 1..=8 {
                let _ = writeln!(content, "    work_{i}_{step}");
            }
            content.push_str("  end\n\n");
        }
        content.push_str("end\n");
        content
    }

    fn ruby() -> Splitter {
        Splitter::new(&[ChunkBoundary {
            extensions: vec!["rb".to_string()],
            pattern: r"^\s*def\s".to_string(),
        }])
    }

    #[test]
    fn a_file_that_fits_is_given_whole_and_numbered() {
        let out = ruby().excerpt(Path::new("a.rb"), "x\ny\n", 1_000, &Focus::default());
        assert_eq!(out, "   1 | x\n   2 | y\n");
    }

    #[test]
    fn a_long_file_shows_its_header_and_the_chunks_naming_the_focus() {
        let content = controller();
        let focus = Focus {
            terms: vec!["target_a".to_string()],
            ..Focus::default()
        };

        let out = ruby().excerpt(Path::new("c.rb"), &content, 4_000, &focus);

        assert!(out.len() <= 4_200, "{}", out.len());
        assert!(out.contains("| class C"), "header kept");
        assert!(out.contains("def target_a"), "the focused method is shown");
        assert!(!out.contains("def target_b"));
        let line = out.lines().find(|l| l.contains("def target_a")).unwrap();
        assert!(line.trim_start().starts_with("321 |"), "{line}");
        assert!(out.contains("omitted)"));
    }

    #[test]
    fn cited_lines_outweigh_names_and_no_focus_keeps_the_beginning() {
        let content = controller();
        let cited = Focus {
            terms: vec!["target_a".to_string()],
            lines: vec![(123, 125)],
            ..Focus::default()
        };
        let out = ruby().excerpt(Path::new("c.rb"), &content, 4_000, &cited);
        assert!(out.contains("def target_b"), "{out}");

        let head = ruby().excerpt(Path::new("c.rb"), &content, 4_000, &Focus::default());
        assert!(head.contains("def action_1\n"));
        assert!(!head.contains("def target_a"));
        assert!(head.trim_end().ends_with("omitted)"));
    }

    #[test]
    fn a_copied_part_marker_is_stripped_from_a_cited_path() {
        assert_eq!(
            strip_part_marker("config/routes.rb (part 1/2)"),
            "config/routes.rb"
        );
        assert_eq!(strip_part_marker(" a/b.rb "), "a/b.rb");
        assert_eq!(strip_part_marker("a/(part)/b.rb"), "a/(part)/b.rb");
        assert_eq!(strip_part_marker("a/b(1).rb"), "a/b(1).rb");
    }
}
