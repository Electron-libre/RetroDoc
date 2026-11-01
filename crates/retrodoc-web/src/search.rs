//! What the search page shows: the hits of the index `search_docs` and
//! `retrodoc search` answer from, up to [`SEARCH_LIMIT`], grouped by kind, or
//! the not-documented answer.

use retrodoc_core::model::ConfidenceScore;
use retrodoc_mcp::corpus::feature_id;
use retrodoc_mcp::search::SEARCH_LIMIT;
use retrodoc_mcp::{Entry, EntryKind, GeneratedDocs};
use retrodoc_render::anchor;

use crate::view::{feature_href, rule_anchor, url_path, use_case_href, Confidence};

/// The kinds in the order of the page, with their heading.
const KINDS: [(EntryKind, &str); 6] = [
    (EntryKind::Domain, "Domains"),
    (EntryKind::Feature, "Features"),
    (EntryKind::UseCase, "Use cases"),
    (EntryKind::Rule, "Business rules"),
    (EntryKind::Glossary, "Glossary"),
    (EntryKind::Doc, "Existing docs"),
];

pub struct SearchPage {
    /// As asked, trimmed.
    pub query: String,
    /// `None` for an empty query.
    pub results: Option<Results>,
}

pub enum Results {
    /// Each kind that has hits, in the order of [`KINDS`].
    Hits(Vec<HitGroup>),
    NotDocumented {
        entries: usize,
        /// Entries that share only the common words of the query.
        left_out: usize,
        /// The words of the query no entry has, as written.
        unknown_words: Vec<String>,
    },
}

pub struct HitGroup {
    pub heading: &'static str,
    /// Best first.
    pub hits: Vec<HitRow>,
}

pub struct HitRow {
    pub title: String,
    /// `None` for a rule whose feature is not in the run.
    pub href: Option<String>,
    pub confidence: Option<Confidence>,
}

/// The search page of `query` over `docs`.
#[must_use]
pub fn search_page(docs: &GeneratedDocs, query: &str) -> SearchPage {
    let query = query.trim().to_string();
    if query.is_empty() {
        return SearchPage {
            query,
            results: None,
        };
    }
    let found = docs.index().search(&query, SEARCH_LIMIT);
    let results = if found.hits.is_empty() {
        Results::NotDocumented {
            entries: docs.index().len(),
            left_out: found.left_out,
            unknown_words: found.unknown_words,
        }
    } else {
        Results::Hits(
            KINDS
                .iter()
                .map(|(kind, heading)| HitGroup {
                    heading,
                    hits: found
                        .hits
                        .iter()
                        .filter(|h| h.entry.kind == *kind)
                        .map(|h| hit_row(docs, h.entry))
                        .collect(),
                })
                .filter(|group| !group.hits.is_empty())
                .collect(),
        )
    };
    SearchPage {
        query,
        results: Some(results),
    }
}

fn hit_row(docs: &GeneratedDocs, entry: &Entry) -> HitRow {
    HitRow {
        title: entry.title.clone(),
        href: href(docs, entry),
        confidence: entry
            .confidence
            .map(|v| Confidence::of(Some(&ConfidenceScore::new(v, None)))),
    }
}

/// The page of the entry: the id of a feature is `<domain>/<feature>`, that of
/// a use case `<domain>/<feature>/<use case>`, that of a rule its slug.
fn href(docs: &GeneratedDocs, entry: &Entry) -> Option<String> {
    match entry.kind {
        EntryKind::Domain => Some(format!("/domains/{}", entry.id)),
        EntryKind::Feature => docs
            .features()
            .iter()
            .find(|f| feature_id(f) == entry.id)
            .map(feature_href),
        EntryKind::UseCase => docs.features().iter().find_map(|f| {
            docs.use_cases_of(&f.slug)
                .find(|u| format!("{}/{}", feature_id(f), u.slug) == entry.id)
                .map(|u| use_case_href(f, u))
        }),
        EntryKind::Rule => {
            let rule = docs.rules().iter().find(|r| r.slug == entry.id)?;
            docs.features()
                .iter()
                .find(|f| rule.belongs_to_feature(&f.slug))
                .map(|f| format!("{}#{}", feature_href(f), rule_anchor(&rule.slug)))
        }
        EntryKind::Glossary => Some(format!("/glossary#{}", anchor(&entry.id))),
        EntryKind::Doc => Some(format!("/docs/{}", url_path(&entry.id))),
    }
}

/// The value of `name` in a query string: `+` read as a space, the
/// percent-escapes decoded; `None` when it is not there.
#[must_use]
pub fn query_value(query: Option<&str>, name: &str) -> Option<String> {
    let raw = query?
        .split('&')
        .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))?;
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => decoded.push(b' '),
            b'%' => {
                let hex = bytes
                    .get(i + 1..i + 3)
                    .filter(|h| h.iter().all(u8::is_ascii_hexdigit))
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                if let Some(byte) = hex {
                    decoded.push(byte);
                    i += 2;
                } else {
                    decoded.push(b'%');
                }
            }
            byte => decoded.push(byte),
        }
        i += 1;
    }
    Some(String::from_utf8_lossy(&decoded).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_value_is_decoded_as_a_form_sends_it() {
        let value = |q: &str| query_value(Some(q), "q");
        assert_eq!(value("q=late+fee%C3%A9").as_deref(), Some("late feeé"));
        assert_eq!(value("x=1&q=a%2Bb").as_deref(), Some("a+b"));
        assert_eq!(value("q=100%").as_deref(), Some("100%"));
        assert_eq!(value("q=%+1").as_deref(), Some("% 1"));
        assert_eq!(value("qq=a"), None);
        assert_eq!(query_value(None, "q"), None);
    }
}
