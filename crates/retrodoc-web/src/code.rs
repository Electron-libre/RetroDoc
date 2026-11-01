//! What the page of a cited file shows: its lines through `SourceAccess`
//! (only the files the docs cite, ADR 0016), its recent commits and what
//! cites it.

use retrodoc_core::model::RuleEvidence;
use retrodoc_mcp::source::{FileCommit, SourceAccess, SourceError, DEFAULT_LOG};
use retrodoc_mcp::GeneratedDocs;

use crate::view::{feature_href, home_crumb, stale_files, use_case_href, Link, StaleFile};

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

/// The cited lines `?lines=<start>-<end>` or `?lines=<start>` asks for, `None`
/// when the query holds no such range.
#[must_use]
pub fn cited_range(query: Option<&str>) -> Option<(usize, usize)> {
    let value = query?
        .split('&')
        .find_map(|pair| pair.strip_prefix("lines="))?;
    let (start, end) = if let Some((start, end)) = value.split_once('-') {
        (start.parse().ok()?, end.parse().ok()?)
    } else {
        let line = value.parse().ok()?;
        (line, line)
    };
    (start >= 1 && end >= start).then_some((start, end))
}

/// The page of the file `path`, its lines `cited` marked; `None` when the
/// path is none the docs cite, read nothing then.
#[must_use]
pub fn code_page(
    docs: &GeneratedDocs,
    source: &SourceAccess,
    path: &str,
    cited: Option<(usize, usize)>,
) -> Option<CodePage> {
    let (start, end) = match cited {
        Some((start, end)) => (
            Some(start.saturating_sub(CONTEXT).max(1)),
            Some(end.saturating_add(CONTEXT)),
        ),
        None => (None, None),
    };
    let (shown, lines, refused) = match source.window(path, start, end) {
        Err(error) if error.not_served() => return None,
        Err(error) => (None, Vec::new(), Some(reason(&error, cited))),
        Ok(window) => (
            Some(format!(
                "lines {}-{} of {}",
                window.start, window.end, window.total
            )),
            window
                .lines
                .into_iter()
                .map(|line| LineView {
                    cited: cited.is_some_and(|(s, e)| (s..=e).contains(&line.number)),
                    number: line.number,
                    text: line.text,
                    cut_from: line.cut_from,
                })
                .collect(),
            None,
        ),
    };
    let path = path.strip_prefix("./").unwrap_or(path);
    Some(CodePage {
        crumbs: vec![home_crumb()],
        path: path.to_string(),
        stale: stale_files(docs.freshness(), [path]),
        shown,
        lines,
        refused,
        commits: source.commits(path, DEFAULT_LOG).ok(),
        cited_by: citers(docs, path),
    })
}

/// Why a cited file can't be shown, for a person rather than an agent: a
/// range past the end names the first cited line, not the window's.
fn reason(error: &SourceError, cited: Option<(usize, usize)>) -> String {
    match error {
        SourceError::PastEnd { total, start, .. } => {
            let line = cited.map_or(*start, |(first, _)| first);
            format!("The file has {total} lines: line {line} is past the end.")
        }
        other => other.to_string(),
    }
}

/// The features, use cases and rules that cite `path`, in the order of the run.
fn citers(docs: &GeneratedDocs, path: &str) -> Vec<Citer> {
    let same = |cited: &str| cited.strip_prefix("./").unwrap_or(cited) == path;
    let mut citers = Vec::new();
    for feature in docs.features() {
        if feature.source_paths.iter().any(|p| same(p)) {
            citers.push(Citer {
                kind: "Feature",
                link: Link {
                    href: feature_href(feature),
                    name: feature.name.clone(),
                },
            });
        }
        for use_case in docs.use_cases_of(&feature.slug) {
            let steps_cite = use_case
                .steps
                .iter()
                .flat_map(|s| &s.source_refs)
                .any(|r| same(&r.path));
            if steps_cite {
                citers.push(Citer {
                    kind: "Use case",
                    link: Link {
                        href: use_case_href(feature, use_case),
                        name: use_case.name.clone(),
                    },
                });
            }
        }
        for rule in docs
            .rules()
            .iter()
            .filter(|r| r.belongs_to_feature(&feature.slug))
        {
            let evidence_cites = rule.evidence.iter().any(|e| match e {
                RuleEvidence::Code { source } => same(&source.path),
                _ => false,
            });
            if evidence_cites {
                citers.push(Citer {
                    kind: "Rule",
                    link: Link {
                        href: feature_href(feature),
                        name: rule.statement.clone(),
                    },
                });
            }
        }
    }
    citers
}
