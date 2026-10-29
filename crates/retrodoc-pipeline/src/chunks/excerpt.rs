//! The use cases and confidence passes send the code to the LLM as a numbered
//! excerpt: for a long file it is no longer its first characters but the
//! chunks that matter ([`Splitter::excerpt`]): the first chunk (the header),
//! then the ones naming what the feature is about or holding the lines a step
//! cites ([`Focus`]), with absolute line numbers and the omitted ranges marked.

use std::fmt::Write as _;
use std::path::Path;

use super::{split_chunks, Splitter};
use crate::text::first_chars;
use crate::use_cases::numbered_excerpt;

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
            out = first_chars(&whole, max_chars).to_string();
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
        for chunk in split_chunks(
            content,
            EXCERPT_CHUNK_CHARS,
            self.boundary_for(path, content).as_ref(),
        ) {
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
