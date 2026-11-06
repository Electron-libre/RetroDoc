//! What the debt page shows (`specs/debt-view.md`): the debt items of the
//! read model, kept by the kind and the domain asked in the query, each with
//! the names of its domain and section and a link to its page.

use retrodoc_core::model::ConfidenceScore;
use retrodoc_mcp::debt::{
    debt, entry_file, DebtItem, DebtKind, DocItem, ItemKind, Section, Subject,
};
use retrodoc_mcp::reviews::Reviews;
use retrodoc_mcp::GeneratedDocs;

use crate::review::OutdatedView;
use crate::texts::WebTexts;
use crate::view::{active_filters, Filter};
use crate::view::{code_href, doc_section_href, Confidence, Link, StaleFile};

/// What the query asks: an empty value asks nothing.
#[derive(Default)]
pub struct DebtFilter {
    pub kind: Option<String>,
    pub domain: Option<String>,
}

pub struct DebtPage {
    /// The kind and the domain, in that order.
    pub filters: Vec<Filter>,
    /// The run has no item at all, whatever the filters.
    pub none_in_run: bool,
    /// The run has no confidence score: no use case is listed as not scored.
    pub no_score: bool,
    /// There is no collected doc: no item is listed as contradicted or undocumented.
    pub no_docs: bool,
    /// Says the file of the reviews could not be read, so the items take no review into account;
    /// `None` when it was.
    pub reviews_note: Option<&'static str>,
    /// Says the git history was not read, so no item counts a commit; `None` when it was, or with no
    /// item.
    pub history_note: Option<&'static str>,
    pub items: Vec<DebtRow>,
}

pub struct DebtRow {
    pub kind: &'static str,
    /// `None` when its feature is not in the run.
    pub domain: Option<String>,
    /// The use case, or the feature.
    pub name: String,
    /// The feature of a use case.
    pub feature: Option<String>,
    /// The page of its section; `None` when its feature is not in the run.
    pub href: Option<String>,
    pub score: Option<Confidence>,
    pub rationale: Option<String>,
    pub reasons: Vec<&'static str>,
    pub changed: Vec<StaleFile>,
    /// Why the review of a section marked wrong or to review is outdated, or why the edit of an
    /// outdated edit is.
    pub outdated: Option<OutdatedView>,
    /// What a doc item is: a domain, a feature, a use case or a rule.
    pub item_kind: Option<&'static str>,
    /// Its priority and its parts, its entry points among them.
    pub priority: String,
    pub contradictions: Vec<ContradictionView>,
    pub folder: Option<FolderView>,
}

/// A section of the project's docs that contradicts an item.
pub struct ContradictionView {
    /// The section on the page of its collected doc.
    pub section: Link,
    pub doc_states: String,
    pub item_states: String,
}

/// A folder of the code no doc covers: its files, or its entry points, each linked to the page of
/// its file.
pub struct FolderView {
    pub path: String,
    /// `3 files`, `1 entry point`.
    pub count: String,
    pub entries: Vec<FolderEntry>,
}

/// A file, or an entry point, of a folder.
pub struct FolderEntry {
    /// The page of its file; `None` for an entry point saved before ids, which names no file.
    pub href: Option<String>,
    pub name: String,
}

/// The debt page of `docs` for `filter`. Reads the cited files.
#[must_use]
pub fn debt_page(docs: &GeneratedDocs, filter: &DebtFilter, texts: &WebTexts) -> DebtPage {
    let reviews = crate::review::load(docs);
    let list = debt(docs, reviews.as_ref().unwrap_or(&Reviews::default()));
    let items = list
        .items
        .iter()
        .filter(|item| filter.keeps(item))
        .map(|item| row(docs, item, texts))
        .collect();
    DebtPage {
        filters: filters(docs, filter, texts),
        none_in_run: list.items.is_empty(),
        no_score: list.no_score,
        no_docs: list.no_docs,
        reviews_note: reviews.is_err().then_some(texts.debt.reviews_unreadable),
        history_note: (docs.churn().is_none() && !list.items.is_empty())
            .then_some(texts.debt.no_history),
        items,
    }
}

impl DebtPage {
    /// The filters the query gives a value.
    #[must_use]
    pub fn active(&self) -> usize {
        active_filters(&self.filters)
    }
}

impl DebtFilter {
    fn keeps(&self, item: &DebtItem) -> bool {
        let kind = self
            .kind
            .as_deref()
            .is_none_or(|asked| item.kind.key() == asked);
        let domain = self
            .domain
            .as_deref()
            .is_none_or(|asked| item.domain() == Some(asked));
        kind && domain
    }
}

fn row(docs: &GeneratedDocs, item: &DebtItem, texts: &WebTexts) -> DebtRow {
    let named = match &item.subject {
        Subject::Section(section) => section_row(docs, section),
        Subject::Domain(slug) => domain_row(docs, slug),
        Subject::DocItem(doc_item) => doc_item_row(docs, doc_item),
        Subject::Folder(_) => Named {
            domain: None,
            name: item.label(),
            feature: None,
            href: None,
        },
    };
    let (item_kind, contradictions) = match &item.subject {
        Subject::DocItem(doc_item) => (
            Some(texts.item_kind(doc_item.kind)),
            doc_item
                .contradictions
                .iter()
                .map(|c| ContradictionView {
                    section: Link {
                        href: doc_section_href(&c.section.path, c.section.heading.as_deref()),
                        name: c.section.origin(),
                    },
                    doc_states: c.doc_states.clone(),
                    item_states: c.item_states.clone(),
                })
                .collect(),
        ),
        _ => (None, Vec::new()),
    };
    let folder = match &item.subject {
        Subject::Folder(folder) => Some(FolderView {
            path: folder.path.clone(),
            count: texts.counted(
                item.kind == DebtKind::EntryPointWithoutFeature,
                folder.entries.len(),
            ),
            entries: folder
                .entries
                .iter()
                .map(|entry| FolderEntry {
                    href: entry_file(entry).map(|file| code_href(file, None, None)),
                    name: entry.clone(),
                })
                .collect(),
        }),
        _ => None,
    };
    DebtRow {
        kind: texts.debt_kind(item.kind),
        domain: named.domain,
        name: named.name,
        feature: named.feature,
        href: named.href,
        score: item
            .score
            .map(|value| Confidence::of(Some(&ConfidenceScore::new(value, None)))),
        rationale: item
            .rationale
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(str::to_string),
        reasons: item
            .reasons
            .iter()
            .map(|r| texts.set_apart_reason(*r))
            .collect(),
        changed: item
            .changed
            .iter()
            .map(|(path, state)| StaleFile::of(path, *state))
            .collect(),
        outdated: item
            .outdated
            .as_ref()
            .map(if item.kind == DebtKind::OutdatedEdit {
                OutdatedView::of_edit
            } else {
                OutdatedView::of
            }),
        item_kind,
        priority: texts.priority(&item.priority),
        contradictions,
        folder,
    }
}

/// How a row names its subject.
struct Named {
    domain: Option<String>,
    name: String,
    feature: Option<String>,
    href: Option<String>,
}

/// The names and the page of a doc item, from the run; its name and no link when its domain or
/// feature is not in the run.
fn doc_item_row(docs: &GeneratedDocs, item: &DocItem) -> Named {
    let parts: Vec<&str> = item.label.split('/').collect();
    let domain = docs.domains().iter().find(|d| d.slug == item.domain);
    let feature = parts
        .get(1)
        .and_then(|slug| crate::view::find_feature(docs, &item.domain, slug));
    let href = match (item.kind, parts.get(2)) {
        (ItemKind::Domain, _) => domain.map(|d| crate::view::domain_href(&d.slug)),
        (ItemKind::Feature, _) => feature.map(crate::view::feature_href),
        (ItemKind::UseCase, Some(slug)) => feature.and_then(|f| {
            docs.use_cases_of(&f.slug)
                .find(|u| u.slug == *slug)
                .map(|u| crate::view::use_case_href(f, u))
        }),
        (ItemKind::Rule, Some(slug)) => feature.map(|f| crate::view::rule_href(f, slug)),
        _ => None,
    };
    Named {
        domain: domain.map_or_else(
            || Some(item.domain.clone()).filter(|d| d != "?"),
            |d| Some(d.name.clone()),
        ),
        name: item.name.clone(),
        feature: feature
            .filter(|_| matches!(item.kind, ItemKind::UseCase | ItemKind::Rule))
            .map(|f| f.name.clone()),
        href,
    }
}

/// The name and the page of a domain of the run.
fn domain_row(docs: &GeneratedDocs, slug: &str) -> Named {
    let name = docs
        .domains()
        .iter()
        .find(|d| d.slug == slug)
        .map_or_else(|| slug.to_string(), |d| d.name.clone());
    Named {
        domain: Some(name.clone()),
        name,
        feature: None,
        href: Some(crate::view::domain_href(slug)),
    }
}

/// The names and the page of a section, from the run; its label and no link when its feature is
/// not in the run.
fn section_row(docs: &GeneratedDocs, section: &Section) -> Named {
    let feature = section
        .domain
        .as_deref()
        .and_then(|domain| crate::view::find_feature(docs, domain, &section.feature));
    let domain = feature.map(|f| {
        docs.domains()
            .iter()
            .find(|d| d.slug == f.domain_slug)
            .map_or_else(|| f.domain_slug.clone(), |d| d.name.clone())
    });
    let use_case = section
        .use_case
        .as_deref()
        .map(|slug| docs.use_cases_of(&section.feature).find(|u| u.slug == slug));
    let (name, href) = match (feature, use_case) {
        (Some(f), None) => (f.name.clone(), Some(crate::view::feature_href(f))),
        (Some(f), Some(Some(u))) => (u.name.clone(), Some(crate::view::use_case_href(f, u))),
        _ => (section.label(), None),
    };
    Named {
        domain,
        name,
        feature: feature
            .filter(|_| section.use_case.is_some())
            .map(|f| f.name.clone()),
        href,
    }
}

fn filters(docs: &GeneratedDocs, filter: &DebtFilter, texts: &WebTexts) -> Vec<Filter> {
    vec![
        Filter::new(
            "kind",
            texts.kind,
            true,
            filter.kind.as_deref(),
            DebtKind::ALL
                .iter()
                .map(|kind| (kind.key(), texts.debt_kind(*kind))),
        ),
        Filter::new(
            "domain",
            texts.domain,
            false,
            filter.domain.as_deref(),
            docs.domains()
                .iter()
                .map(|d| (d.slug.as_str(), d.name.as_str())),
        ),
    ]
}
