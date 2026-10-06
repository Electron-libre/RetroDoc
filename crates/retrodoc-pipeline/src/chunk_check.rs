//! Mechanical check of the `chunk_boundaries` regexes the LLM proposes
//! (see `chunks.rs`): a regex is only a good place to cut if it recognizes
//! the lines that start definitions in the repo's real files.
//!
//! A language-agnostic probe picks, in a sample of the repo's source files of
//! the rule's extensions, the lines that look like a definition (optional
//! modifiers then a keyword such as `fn`, `def`, `function`, `class`, `impl`…)
//! and the decorator/attribute lines right above them. The share of those
//! lines the LLM regex matches is its coverage. Below [`GOOD_COVERAGE`] the
//! LLM gets one more, focused call with the lines it missed; the better of
//! the two regexes is kept, and dropped altogether below [`MIN_COVERAGE`]
//! (the chunker then falls back to blank lines). A language whose files show
//! no definition-like line can't be checked and is kept as proposed.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use retrodoc_ingest::{FileEntry, FileKind};
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use crate::chunks::{ChunkBoundary, REGEX_SYNTAX_HELP};
use crate::error::PipelineError;
use crate::repo_map::read_file_lossy;
use crate::response::complete_json;

/// Coverage from which a regex is accepted without a second call.
const GOOD_COVERAGE: f64 = 0.9;
/// Focused calls to improve a weak regex, each told what the best one so far misses.
const MAX_FIX_ATTEMPTS: u32 = 3;
/// Below this even the better regex is dropped.
const MIN_COVERAGE: f64 = 0.5;
/// Largest files per rule read for the probe (the ones that get chunked).
const SAMPLE_FILES: usize = 20;
/// A regex matching more than this share of all the sampled lines is not
/// picking out definitions (an empty alternative `a|b|` matches everything).
const MAX_BREADTH: f64 = 0.4;
/// Cap on the sampled lines kept to measure the breadth.
const MAX_SAMPLE_LINES: usize = 50_000;
/// Cap on the expected lines gathered per rule.
const MAX_EXPECTED_LINES: usize = 3_000;
const MISSED_EXAMPLES: usize = 8;
const MAX_EXAMPLE_CHARS: usize = 120;

const FIX_SYSTEM_PROMPT: &str = "You write a regular expression (Rust regex syntax, tried on one \
source line at a time) that matches every line that STARTS a module, class or function/method \
definition in a programming language, and the decorator, attribute or annotation lines right \
before a definition, never the line that ends it. Allow indentation and every modifier that can \
precede the keyword (visibility such as `pub(crate)` or `public static`, `async`, `export \
default`, `abstract`...). Reply with ONLY a single JSON object, no prose and no Markdown code \
fence, matching this shape: {\"pattern\":\"...\"}.";

/// A line that looks like the start of a definition, whatever the language.
static DEFINITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?:(?:pub|public|private|protected|internal|static|async|export|default|abstract|final|unsafe|const|extern|override|virtual|open|data|sealed|partial|inline|suspend|readonly)(?:\([\w:]+\))?\s+)*(?:fn|def|function|func|fun|class|struct|enum|interface|trait|impl|module|mod|namespace|object|protocol|sub)(?:\s|<)",
    )
    .expect("valid probe regex")
});

#[derive(Debug, Deserialize)]
struct PatternResponse {
    pattern: String,
}

/// How well a regex covers the expected lines.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Coverage {
    pub ratio: f64,
    /// Why the regex is unusable (doesn't compile, matches far too much), if
    /// it is; its `ratio` is then 0.
    pub problem: Option<String>,
    pub missed: Vec<String>,
}

/// Lines that should start a chunk: definition-like lines, and the decorator
/// or attribute line right above one (`#[derive(..)]`, `@Override`).
pub(crate) fn expected_lines<'a>(contents: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut expected = Vec::new();
    for content in contents {
        let mut previous: Option<&str> = None;
        for line in content.split_inclusive('\n') {
            if DEFINITION.is_match(line) {
                if let Some(above) = previous.filter(|l| is_decorator(l)) {
                    expected.push(above.to_string());
                }
                expected.push(line.to_string());
            }
            previous = Some(line);
            if expected.len() >= MAX_EXPECTED_LINES {
                return expected;
            }
        }
    }
    expected
}

/// Every line of the sample (capped), to measure how much a regex matches.
pub(crate) fn sample_lines<'a>(contents: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    contents
        .into_iter()
        .flat_map(|content| content.split_inclusive('\n'))
        .take(MAX_SAMPLE_LINES)
        .map(str::to_string)
        .collect()
}

fn is_decorator(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("#[") || (line.starts_with('@') && !line.contains('='))
}

/// Share of `expected` matched by `pattern`, with a few distinct missed lines.
/// It is 0 when the regex doesn't compile or matches more than [`MAX_BREADTH`]
/// of `lines` (the whole sample). `None` when there is nothing to check.
pub(crate) fn coverage(pattern: &str, expected: &[String], lines: &[String]) -> Option<Coverage> {
    if expected.is_empty() {
        return None;
    }
    let compiled = Regex::new(pattern);
    let mut problem = compiled.as_ref().err().map(|e| {
        // The first line of the message names the problem; the rest repeats the pattern.
        e.to_string()
            .lines()
            .last()
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    let regex = compiled.ok();
    if let Some(regex) = &regex {
        #[allow(clippy::cast_precision_loss)]
        let breadth =
            lines.iter().filter(|l| regex.is_match(l)).count() as f64 / lines.len().max(1) as f64;
        if breadth > MAX_BREADTH {
            problem = Some(format!(
                "it matches {:.0}% of all the lines of the sample (blank lines, statements, \
                 comments...) instead of only the lines that start a definition; look for an \
                 empty alternative such as a trailing `|`",
                breadth * 100.0
            ));
        }
    }
    let regex = regex.filter(|_| problem.is_none());
    let mut matched = 0_usize;
    let mut missed: Vec<String> = Vec::new();
    for line in expected {
        if regex.as_ref().is_some_and(|r| r.is_match(line)) {
            matched += 1;
        } else if missed.len() < MISSED_EXAMPLES {
            let example: String = line.trim().chars().take(MAX_EXAMPLE_CHARS).collect();
            if !missed.contains(&example) {
                missed.push(example);
            }
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let ratio = matched as f64 / expected.len() as f64;
    Some(Coverage {
        ratio,
        problem,
        missed,
    })
}

/// Checks every proposed rule against the repo's files, asks the LLM (up to
/// [`MAX_FIX_ATTEMPTS`] times) to fix those below [`GOOD_COVERAGE`], and keeps the better regex (dropping
/// the rule if even that is below [`MIN_COVERAGE`]).
///
/// # Errors
///
/// Returns an error if an LLM call fails.
pub(crate) async fn verify_boundaries(
    repo_root: &Path,
    files: &[FileEntry],
    llm: &dyn LlmProvider,
    proposed: Vec<ChunkBoundary>,
) -> Result<Vec<ChunkBoundary>, PipelineError> {
    let mut verified = Vec::new();
    for mut boundary in proposed {
        let contents = sample_contents(repo_root, files, &boundary.extensions);
        let expected = expected_lines(contents.iter().map(String::as_str));
        let lines = sample_lines(contents.iter().map(String::as_str));
        let Some(mut best) = coverage(&boundary.pattern, &expected, &lines) else {
            tracing::info!(
                extensions = ?boundary.extensions,
                "chunk boundary not checkable (no definition-like line in the sample), kept"
            );
            verified.push(boundary);
            continue;
        };
        for attempt in 1..=MAX_FIX_ATTEMPTS {
            if best.ratio >= GOOD_COVERAGE {
                break;
            }
            tracing::warn!(
                extensions = ?boundary.extensions,
                pattern = %boundary.pattern,
                coverage = format!("{:.0}%", best.ratio * 100.0),
                attempt,
                "chunk boundary misses definitions, asking the LLM to fix it"
            );
            let Some(fixed) = ask_fix(llm, &boundary, &best).await? else {
                continue;
            };
            let Some(candidate) = coverage(&fixed, &expected, &lines) else {
                continue;
            };
            tracing::info!(
                pattern = %fixed,
                coverage = format!("{:.0}%", candidate.ratio * 100.0),
                "chunk boundary fix proposed"
            );
            if candidate.ratio > best.ratio {
                boundary.pattern = fixed;
                best = candidate;
            }
        }
        if best.ratio < MIN_COVERAGE {
            tracing::warn!(
                extensions = ?boundary.extensions,
                coverage = format!("{:.0}%", best.ratio * 100.0),
                "chunk boundary dropped, long files will be cut at blank lines"
            );
            continue;
        }
        tracing::info!(
            extensions = ?boundary.extensions,
            coverage = format!("{:.0}%", best.ratio * 100.0),
            "chunk boundary checked"
        );
        verified.push(boundary);
    }
    Ok(verified)
}

/// Contents of the largest source files with one of `extensions` (a file
/// that can't be read is left out of the sample).
fn sample_contents(repo_root: &Path, files: &[FileEntry], extensions: &[String]) -> Vec<String> {
    let wanted: Vec<String> = extensions
        .iter()
        .map(|e| e.trim_start_matches('.').to_lowercase())
        .collect();
    let mut candidates: Vec<&FileEntry> = files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .filter(|f| {
            f.path
                .extension()
                .is_some_and(|e| wanted.contains(&e.to_string_lossy().to_lowercase()))
        })
        .collect();
    candidates.sort_by_key(|f| std::cmp::Reverse(f.size_bytes));
    candidates
        .into_iter()
        .take(SAMPLE_FILES)
        .filter_map(|f| read_file_lossy(repo_root, &f.path).ok())
        .collect()
}

async fn ask_fix(
    llm: &dyn LlmProvider,
    boundary: &ChunkBoundary,
    coverage: &Coverage,
) -> Result<Option<String>, PipelineError> {
    let mut prompt = format!("Language files: {}\n", boundary.extensions.join(", "));
    if let Some(problem) = &coverage.problem {
        let _ = writeln!(
            prompt,
            "Your regex `{}` is unusable: {problem}. Write one that follows the rules.",
            boundary.pattern
        );
    }
    let _ = writeln!(
        prompt,
        "Your regex `{}` matches only {:.0}% of the lines that start a \
         definition or sit right above one. It misses lines like:",
        boundary.pattern,
        coverage.ratio * 100.0
    );
    for line in &coverage.missed {
        prompt.push_str("  ");
        prompt.push_str(line);
        prompt.push('\n');
    }
    prompt.push_str("Write a regex that matches all of them (and the lines it already matches).");
    let system_prompt = format!("{FIX_SYSTEM_PROMPT} {REGEX_SYNTAX_HELP}");
    Ok(
        complete_json::<PatternResponse>(llm, &system_prompt, &prompt, "chunk boundary fix")
            .await?
            .map(|r| r.pattern),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testing::FakeLlm;

    const RUST: &str = "#[derive(Debug)]
struct A;

impl A {
    pub(crate) fn new() -> Self {
        let a = 1;
        let b = 2;
        let c = a + b;
        println!(\"{c}\");
        A
    }
    async fn run(&self) {
        let a = 1;
        let b = 2;
        let c = a + b;
        println!(\"{c}\");
    }
}
let x = module.exports;
";

    #[test]
    fn the_probe_finds_definitions_and_the_attribute_above_them() {
        let expected = expected_lines([RUST]);
        let trimmed: Vec<&str> = expected.iter().map(|l| l.trim()).collect();
        assert_eq!(
            trimmed,
            vec![
                "#[derive(Debug)]",
                "struct A;",
                "impl A {",
                "pub(crate) fn new() -> Self {",
                "async fn run(&self) {",
            ]
        );
    }

    #[test]
    fn coverage_counts_matched_lines_and_lists_misses() {
        let expected = expected_lines([RUST]);
        let lines = sample_lines([RUST]);
        let narrow = coverage(r"^\s*(pub )?(fn|struct|impl)\s", &expected, &lines).unwrap();
        assert!((narrow.ratio - 0.4).abs() < 1e-9, "{}", narrow.ratio);
        assert!(narrow.missed.iter().any(|l| l.starts_with("async fn")));
        let wide = coverage(
            r"^\s*(#\[|(pub(\(\w+\))?\s+)?(async\s+)?(fn|struct|impl)\s)",
            &expected,
            &lines,
        )
        .unwrap();
        assert!((wide.ratio - 1.0).abs() < 1e-9);
        let broken = coverage("(", &expected, &lines).unwrap();
        assert!(broken.ratio < 1e-9);
        assert!(broken.problem.is_some());
        assert!(coverage("x", &[], &lines).is_none());
    }

    #[test]
    fn a_regex_that_matches_everything_is_rejected_whatever_its_recall() {
        let expected = expected_lines([RUST]);
        let lines = sample_lines([RUST]);
        // The trailing `|` is an empty alternative: it matches every line.
        let sloppy = coverage(r"^\s*(fn|struct|impl)\s|", &expected, &lines).unwrap();
        assert!(sloppy.ratio < 1e-9);
        assert!(sloppy.problem.unwrap().contains("empty alternative"));
    }

    fn rust_repo() -> (tempfile::TempDir, Vec<FileEntry>) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), RUST).unwrap();
        let files = vec![FileEntry {
            path: "a.rs".into(),
            kind: FileKind::Source,
            size_bytes: 10,
        }];
        (dir, files)
    }

    fn boundary(pattern: &str) -> ChunkBoundary {
        ChunkBoundary {
            extensions: vec!["rs".to_string()],
            pattern: pattern.to_string(),
        }
    }

    #[tokio::test]
    async fn a_weak_regex_is_fixed_by_one_more_call_and_a_good_one_is_left_alone() {
        let (dir, files) = rust_repo();
        let fix = r"^\s*(#\[|(pub(\(\w+\))?\s+)?(async\s+)?(fn|struct|impl)\s)";
        let llm = FakeLlm::answering(format!(r#"{{"pattern":{fix:?}}}"#));

        let out = verify_boundaries(dir.path(), &files, &llm, vec![boundary(r"^\s*fn\s")])
            .await
            .unwrap();
        assert_eq!(out[0].pattern, fix);
        assert_eq!(llm.calls(), 1);

        let out = verify_boundaries(dir.path(), &files, &llm, vec![boundary(fix)])
            .await
            .unwrap();
        assert_eq!(out[0].pattern, fix);
        assert_eq!(llm.calls(), 1);
    }

    #[tokio::test]
    async fn a_rule_that_stays_bad_is_dropped_and_an_uncheckable_one_kept() {
        let (dir, files) = rust_repo();
        let llm = FakeLlm::answering(r#"{"pattern":"zzz"}"#);
        let other = ChunkBoundary {
            extensions: vec!["lisp".to_string()],
            pattern: "^\\(defun".to_string(),
        };

        let out = verify_boundaries(dir.path(), &files, &llm, vec![boundary("nothing"), other])
            .await
            .unwrap();

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].extensions, vec!["lisp"]);
    }
}
