//! The sections of the project's own docs that document or contradict an item
//! (`specs/docs-reconciliation.md`): an "Existing documentation" section on the page of a domain, a
//! feature or a use case, a line under a rule. The links are relative from the page to the doc file,
//! so they resolve in the repository as on a forge.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::markdown::link;

/// The item a doc section is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemRef {
    Domain(String),
    Feature(String),
    UseCase { feature: String, use_case: String },
    Rule(String),
}

/// A doc section behind an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocLink {
    /// From the repository root.
    pub path: String,
    pub heading: Option<String>,
    /// What the section states against the item, when it contradicts it.
    pub contradicts: Option<String>,
}

/// The doc sections of each item, and where the docs are written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocLinks {
    /// The docs dir, from the repository root.
    pub docs_dir: PathBuf,
    pub links: BTreeMap<ItemRef, Vec<DocLink>>,
}

impl DocLinks {
    pub(crate) fn of(&self, item: &ItemRef) -> &[DocLink] {
        self.links.get(item).map_or(&[], Vec::as_slice)
    }

    /// `- [`path`, section "heading"](link)`, flagged when it contradicts, from the page `from`
    /// (relative to the docs dir).
    fn item(&self, doc: &DocLink, from: &Path) -> String {
        // `./docs` and `docs` are the same dir: a `.` would count as one more level.
        let docs_dir: PathBuf = self
            .docs_dir
            .components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect();
        let target = link(&docs_dir.join(from), Path::new(&doc.path));
        let (label, target) = match &doc.heading {
            Some(h) => (
                format!("`{}`, section \"{}\"", doc.path, one_line(h)),
                format!("{target}#{}", anchor(h)),
            ),
            None => (format!("`{}`", doc.path), target),
        };
        let mut item = format!("[{label}]({target})");
        if let Some(states) = &doc.contradicts {
            let _ = write!(item, " ⚠️ states otherwise: {}", one_line(states));
        }
        item
    }

    /// The "Existing documentation" section of the page `from`; nothing for an item no section
    /// documents.
    pub(crate) fn section(&self, item: &ItemRef, from: &Path) -> String {
        let docs = self.of(item);
        if docs.is_empty() {
            return String::new();
        }
        let mut md = String::from("## Existing documentation\n\n");
        for doc in docs {
            let _ = writeln!(md, "- {}", self.item(doc, from));
        }
        md.push('\n');
        md
    }

    /// The lines under a rule in a list of the page `from`.
    pub(crate) fn rule_lines(&self, rule_slug: &str, from: &Path) -> String {
        let mut md = String::new();
        for doc in self.of(&ItemRef::Rule(rule_slug.to_string())) {
            let _ = writeln!(md, "  - Existing doc: {}", self.item(doc, from));
        }
        md
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The anchor a forge gives a heading: lowercase, spaces as `-`, punctuation dropped.
fn anchor(heading: &str) -> String {
    heading
        .trim()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c.to_lowercase().collect::<String>())
            } else if c == ' ' {
                Some("-".to_string())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: REC-12
    #[test]
    fn a_docs_dir_written_from_the_current_dir_links_the_same() {
        let doc = DocLink {
            path: "docs/guide.md".into(),
            heading: Some("Paying".into()),
            contradicts: None,
        };
        let links = |dir: &str| DocLinks {
            docs_dir: PathBuf::from(dir),
            ..DocLinks::default()
        };
        let from = Path::new("functional/billing/pay.md");
        assert_eq!(
            links("./docs").item(&doc, from),
            links("docs").item(&doc, from)
        );
        assert_eq!(
            links(".").item(&doc, from),
            "[`docs/guide.md`, section \"Paying\"](../../docs/guide.md#paying)"
        );
    }
}
