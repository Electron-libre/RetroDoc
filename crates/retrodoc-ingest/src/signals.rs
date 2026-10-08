//! Signals: the non-code evidence the product brief (ADR 0019) is written from.
//! Each signal keeps its origin (file and heading, later commit...) so a claim
//! of the brief can cite it. Collection is deterministic, no LLM.

use std::path::Path;

use crate::error::IngestError;
use crate::existing_docs::{self, ExistingDoc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    /// A section of a Markdown document (README, changelog, `docs/`).
    DocSection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub kind: SignalKind,
    /// Where it comes from, citable: `path` or `path#Heading`.
    pub origin: String,
    /// The heading line followed by the body of the section.
    pub text: String,
}

/// The Markdown files at the root of the repo (README, CHANGELOG...), read
/// as documents whatever `existing_docs_paths` says.
///
/// # Errors
///
/// Returns an error if the root folder is unreadable. A Markdown file that
/// can't be read (permissions, not UTF-8) is skipped with a warning.
pub fn root_markdown(repo_root: &Path) -> Result<Vec<ExistingDoc>, IngestError> {
    let mut names = Vec::new();
    let entries = std::fs::read_dir(repo_root).map_err(|source| IngestError::Read {
        path: repo_root.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let path = entry
            .map_err(|source| IngestError::Read {
                path: repo_root.to_path_buf(),
                source,
            })?
            .path();
        if path.is_file()
            && matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("md" | "mdx")
            )
        {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    let mut docs = Vec::new();
    for name in names {
        match existing_docs::load_existing_docs(repo_root, std::slice::from_ref(&name)) {
            Ok(mut found) => docs.append(&mut found),
            Err(error) => tracing::warn!("skipping root document {name}: {error}"),
        }
    }
    Ok(docs)
}

/// Splits documents into one signal per non-empty section, cut at each
/// Markdown heading (a `#` inside a fenced code block is not one). The text
/// before the first heading is a section named after the file. A document
/// found twice (configured path and root) counts once.
#[must_use]
pub fn doc_sections(docs: &[ExistingDoc]) -> Vec<Signal> {
    let mut seen = std::collections::HashSet::new();
    let mut signals = Vec::new();
    for doc in docs.iter().filter(|d| seen.insert(d.path.clone())) {
        let path = doc.path.to_string_lossy().replace('\\', "/");
        let mut heading: Option<String> = None;
        let mut body = String::new();
        let mut fence: Option<(char, usize)> = None;
        let mut flush = |heading: &Option<String>, body: &mut String| {
            let text = body.trim();
            if !text.is_empty() {
                signals.push(match heading {
                    Some(h) => Signal {
                        kind: SignalKind::DocSection,
                        origin: format!("{path}#{}", h.trim_start_matches('#').trim()),
                        text: format!("{h}\n{text}"),
                    },
                    None => Signal {
                        kind: SignalKind::DocSection,
                        origin: path.clone(),
                        text: text.to_string(),
                    },
                });
            }
            body.clear();
        };
        let content = doc.content.replace("\r\n", "\n");
        for line in strip_front_matter(&content).lines() {
            if let Some((marker, len)) = fence_marker(line) {
                match fence {
                    None => fence = Some((marker, len)),
                    Some((open, open_len))
                        if marker == open
                            && len >= open_len
                            && line.trim().chars().all(|c| c == marker) =>
                    {
                        fence = None;
                    }
                    Some(_) => {}
                }
            }
            if fence.is_none() && is_heading(line) {
                flush(&heading, &mut body);
                heading = Some(line.trim().to_string());
            } else {
                body.push_str(line);
                body.push('\n');
            }
        }
        flush(&heading, &mut body);
    }
    signals
}

/// The fence character and length of a line opening or closing a fenced code
/// block (three backticks or tildes at least).
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let line = line.trim_start();
    let marker = line.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let len = line.chars().take_while(|c| *c == marker).count();
    (len >= 3).then_some((marker, len))
}

fn is_heading(line: &str) -> bool {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes) && line[hashes..].starts_with(' ')
}

fn strip_front_matter(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("---\n") else {
        return content;
    };
    rest.split_once("\n---\n")
        .map_or(content, |(_, after)| after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn doc(path: &str, content: &str) -> ExistingDoc {
        ExistingDoc {
            path: PathBuf::from(path),
            content: content.to_string(),
        }
    }

    #[test]
    fn splits_a_document_at_each_heading() {
        let signals = doc_sections(&[doc(
            "README.md",
            "Intro line\n\n# Title\nAbout it.\n\n## Usage\nRun it.\n",
        )]);
        let origins: Vec<_> = signals.iter().map(|s| s.origin.as_str()).collect();
        assert_eq!(origins, ["README.md", "README.md#Title", "README.md#Usage"]);
        assert_eq!(signals[2].text, "## Usage\nRun it.");
        assert!(signals.iter().all(|s| s.kind == SignalKind::DocSection));
    }

    #[test]
    fn a_hash_in_a_code_fence_is_not_a_heading() {
        let signals = doc_sections(&[doc("a.md", "# Setup\n```sh\n# install\nmake\n```\nDone.\n")]);
        assert_eq!(signals.len(), 1);
        assert!(signals[0].text.contains("# install"));
    }

    #[test]
    fn a_fence_closes_only_with_its_own_marker() {
        let signals = doc_sections(&[doc(
            "a.md",
            "# A\n~~~\n```\n# not a heading\n~~~\nafter\n````md\n```\n# nor this\n````\n",
        )]);
        assert_eq!(signals.len(), 1);
    }

    #[test]
    fn crlf_front_matter_is_stripped() {
        let signals = doc_sections(&[doc("a.md", "---\r\ntitle: x\r\n---\r\n# A\r\nText.\r\n")]);
        assert_eq!(signals.len(), 1);
        assert!(!signals[0].text.contains("title"));
    }

    #[test]
    fn root_markdown_skips_an_unreadable_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("BAD.md"), [0xff, 0xfe, 0x00]).unwrap();
        fs::write(dir.path().join("README.md"), "# r").unwrap();
        let docs = root_markdown(dir.path()).unwrap();
        assert_eq!(docs.len(), 1);
    }

    #[test]
    fn empty_sections_and_front_matter_are_dropped() {
        let signals = doc_sections(&[doc(
            "a.md",
            "---\ntitle: x\n---\n# Parent\n## Child\nText.\n#NotAHeading\n",
        )]);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].origin, "a.md#Child");
        assert!(signals[0].text.contains("#NotAHeading"));
        assert!(!signals[0].text.contains("title: x"));
    }

    #[test]
    fn a_document_listed_twice_counts_once() {
        let d = doc("README.md", "# A\nText.\n");
        assert_eq!(doc_sections(&[d.clone(), d]).len(), 1);
    }

    #[test]
    fn root_markdown_reads_only_root_level_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("README.md"), "# r").unwrap();
        fs::write(root.join("CHANGELOG.md"), "# c").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        fs::create_dir(root.join("notes")).unwrap();
        fs::write(root.join("notes/deep.md"), "# d").unwrap();

        let paths: Vec<_> = root_markdown(root)
            .unwrap()
            .into_iter()
            .map(|d| d.path.to_string_lossy().to_string())
            .collect();
        assert_eq!(paths, ["CHANGELOG.md", "README.md"]);
    }
}
