//! What the page of a cited file shows: its lines through `SourceAccess`
//! (only the files the docs cite, ADR 0016), its recent commits and what
//! cites it.

use std::collections::BTreeSet;

use retrodoc_core::model::RuleEvidence;
use retrodoc_mcp::source::{FileCommit, SourceAccess, SourceError, DEFAULT_LOG};
use retrodoc_mcp::GeneratedDocs;

use crate::view::{
    feature_href, home_crumb, rule_anchor, stale_files, url_path, use_case_href, Link, StaleFile,
};

/// Lines shown before and after the cited ones.
const CONTEXT: usize = 10;

/// The page of a cited file.
pub struct CodePage {
    pub crumbs: Vec<Link>,
    pub path: String,
    pub stale: Vec<StaleFile>,
    /// `lines 30-52 of 300`, `None` when no line is shown.
    pub shown: Option<String>,
    pub lines: Vec<LineView>,
    /// The file has no line.
    pub empty: bool,
    /// The page of the lines after the window, when the file goes on.
    pub next: Option<String>,
    /// Why the file can't be shown.
    pub refused: Option<String>,
    /// `None` when the repository has no readable history.
    pub commits: Option<Vec<FileCommit>>,
    pub cited_by: Vec<Citer>,
}

pub struct LineView {
    pub number: usize,
    pub text: String,
    /// The length of a line cut, in characters.
    pub cut_from: Option<usize>,
    pub cited: bool,
}

/// A feature, a use case or a rule that cites the file.
pub struct Citer {
    pub kind: &'static str,
    pub link: Link,
}

/// The lines a page of a file asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// From the first line.
    Top,
    /// `?lines=<start>-<end>` or `?lines=<start>`: marked, with lines around.
    Cited(usize, usize),
    /// `?from=<line>`: the window from that line, nothing marked.
    From(usize),
}

impl Asked {
    /// What `query` asks for: a valid `lines` first, then a valid `from`.
    #[must_use]
    pub fn of(query: Option<&str>) -> Self {
        let value = |name: &str| {
            query?
                .split('&')
                .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
        };
        if let Some((start, end)) = value("lines").and_then(range) {
            return Self::Cited(start, end);
        }
        match value("from").and_then(|v| v.parse().ok()) {
            Some(line) if line >= 1 => Self::From(line),
            _ => Self::Top,
        }
    }

    /// The first line asked, past which the file has to go on.
    fn first(self) -> Option<usize> {
        match self {
            Self::Top => None,
            Self::Cited(line, _) | Self::From(line) => Some(line),
        }
    }
}

/// `<start>-<end>` or `<start>`, 1-based, not reversed.
fn range(value: &str) -> Option<(usize, usize)> {
    let (start, end) = if let Some((start, end)) = value.split_once('-') {
        (start.parse().ok()?, end.parse().ok()?)
    } else {
        let line = value.parse().ok()?;
        (line, line)
    };
    (start >= 1 && end >= start).then_some((start, end))
}

/// The page of the file `path` at the lines `asked`; `None` when the path is
/// none the docs cite, read nothing then.
#[must_use]
pub fn code_page(
    docs: &GeneratedDocs,
    source: &SourceAccess,
    path: &str,
    asked: Asked,
) -> Option<CodePage> {
    let (start, end) = match asked {
        Asked::Top => (None, None),
        Asked::Cited(start, end) => (
            Some(start.saturating_sub(CONTEXT).max(1)),
            Some(end.saturating_add(CONTEXT)),
        ),
        Asked::From(line) => (Some(line), None),
    };
    let plain = path.strip_prefix("./").unwrap_or(path);
    let mut page = CodePage {
        crumbs: vec![home_crumb()],
        path: plain.to_string(),
        stale: Vec::new(),
        shown: None,
        lines: Vec::new(),
        empty: false,
        next: None,
        refused: None,
        commits: None,
        cited_by: Vec::new(),
    };
    match source.window(path, start, end) {
        Err(error) if error.not_served() => return None,
        Err(error) => page.refused = Some(reason(&error, asked)),
        // The window starts before the cited lines: they may all be past the end.
        Ok(window) if asked.first().is_some_and(|line| line > window.total) => {
            page.refused = Some(past_end(window.total, asked.first().unwrap_or_default()));
        }
        Ok(window) if window.total == 0 => page.empty = true,
        Ok(window) => {
            page.shown = Some(format!(
                "lines {}-{} of {}",
                window.start, window.end, window.total
            ));
            page.next = (window.end < window.total).then(|| {
                let next = window.end + 1;
                format!("/code/{}?from={next}#L{next}", url_path(plain))
            });
            page.lines = window
                .lines
                .into_iter()
                .map(|line| LineView {
                    cited: matches!(asked, Asked::Cited(s, e) if (s..=e).contains(&line.number)),
                    number: line.number,
                    text: line.text,
                    cut_from: line.cut_from,
                })
                .collect();
        }
    }
    page.stale = stale_files(docs.freshness(), [plain]);
    page.commits = source.commits(plain, DEFAULT_LOG).ok();
    page.cited_by = citations(docs)
        .into_iter()
        .filter(|(_, paths)| paths.contains(plain))
        .map(|(citer, _)| citer)
        .collect();
    Some(page)
}

/// Why a cited file can't be shown, for a person rather than an agent: a
/// range past the end names the first line asked, not the window's.
fn reason(error: &SourceError, asked: Asked) -> String {
    match error {
        SourceError::PastEnd { total, start, .. } => {
            past_end(*total, asked.first().unwrap_or(*start))
        }
        other => other.to_string(),
    }
}

fn past_end(total: usize, line: usize) -> String {
    format!("The file has {total} lines: line {line} is past the end.")
}

/// Each feature, use case and rule with the files it cites (as plain
/// repo-relative paths), in the order of the run: a use case under its
/// feature, a rule under the feature it belongs to.
#[must_use]
pub fn citations(docs: &GeneratedDocs) -> Vec<(Citer, BTreeSet<String>)> {
    let plain = |path: &str| path.strip_prefix("./").unwrap_or(path).to_string();
    let mut citations = Vec::new();
    for feature in docs.features() {
        citations.push((
            Citer {
                kind: "Feature",
                link: Link {
                    href: feature_href(feature),
                    name: feature.name.clone(),
                },
            },
            feature.source_paths.iter().map(|p| plain(p)).collect(),
        ));
        for use_case in docs.use_cases_of(&feature.slug) {
            citations.push((
                Citer {
                    kind: "Use case",
                    link: Link {
                        href: use_case_href(feature, use_case),
                        name: use_case.name.clone(),
                    },
                },
                use_case
                    .steps
                    .iter()
                    .flat_map(|s| &s.source_refs)
                    .map(|r| plain(&r.path))
                    .collect(),
            ));
        }
        for rule in docs
            .rules()
            .iter()
            .filter(|r| r.belongs_to_feature(&feature.slug))
        {
            citations.push((
                Citer {
                    kind: "Rule",
                    link: Link {
                        href: format!("{}#{}", feature_href(feature), rule_anchor(&rule.slug)),
                        name: rule.statement.clone(),
                    },
                },
                rule.evidence
                    .iter()
                    .filter_map(|e| match e {
                        RuleEvidence::Code { source } => Some(plain(&source.path)),
                        _ => None,
                    })
                    .collect(),
            ));
        }
    }
    citations
}
