//! What the debt page shows (`specs/debt-view.md`): the debt items of the
//! read model, kept by the kind and the domain asked in the query, each with
//! the names of its domain and section and a link to its page.

use retrodoc_core::model::ConfidenceScore;
use retrodoc_mcp::debt::{debt, DebtItem, DebtKind, Section};
use retrodoc_mcp::GeneratedDocs;

use crate::rules::{Choice, Filter};
use crate::texts::WebTexts;
use crate::view::{Confidence, StaleFile};

/// The value of `kind` in the query.
fn kind_value(kind: DebtKind) -> &'static str {
    match kind {
        DebtKind::LowConfidence => "low_confidence",
        DebtKind::NotScored => "not_scored",
        DebtKind::UnbackedNarrative => "unbacked_narrative",
        DebtKind::TechnicalLanguage => "technical_language",
        DebtKind::SetApart => "set_apart",
        DebtKind::Stale => "stale",
    }
}

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
}

/// The debt page of `docs` for `filter`. Reads the cited files.
#[must_use]
pub fn debt_page(docs: &GeneratedDocs, filter: &DebtFilter, texts: &WebTexts) -> DebtPage {
    let list = debt(docs);
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
        items,
    }
}

impl DebtFilter {
    fn keeps(&self, item: &DebtItem) -> bool {
        let kind = self
            .kind
            .as_deref()
            .is_none_or(|asked| kind_value(item.kind) == asked);
        let domain = self
            .domain
            .as_deref()
            .is_none_or(|asked| item.section.domain.as_deref() == Some(asked));
        kind && domain
    }
}

fn row(docs: &GeneratedDocs, item: &DebtItem, texts: &WebTexts) -> DebtRow {
    let Section {
        domain,
        feature,
        use_case,
    } = &item.section;
    let feature = domain
        .as_deref()
        .and_then(|domain| crate::view::find_feature(docs, domain, feature));
    let domain_name = feature.map(|f| {
        docs.domains()
            .iter()
            .find(|d| d.slug == f.domain_slug)
            .map_or_else(|| f.domain_slug.clone(), |d| d.name.clone())
    });
    let use_case = use_case.as_deref().map(|slug| {
        docs.use_cases_of(&item.section.feature)
            .find(|u| u.slug == slug)
    });
    let (name, href) = match (feature, use_case) {
        (Some(f), None) => (f.name.clone(), Some(crate::view::feature_href(f))),
        (Some(f), Some(Some(u))) => (u.name.clone(), Some(crate::view::use_case_href(f, u))),
        _ => (item.section.label(), None),
    };
    DebtRow {
        kind: texts.debt_kind(item.kind),
        domain: domain_name,
        name,
        feature: feature
            .filter(|_| item.section.use_case.is_some())
            .map(|f| f.name.clone()),
        href,
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
    }
}

fn filters(docs: &GeneratedDocs, filter: &DebtFilter, texts: &WebTexts) -> Vec<Filter> {
    let choice = |value: &str, label: &str, asked: Option<&str>| Choice {
        value: value.to_string(),
        label: label.to_string(),
        selected: asked == Some(value),
    };
    vec![
        Filter {
            name: "kind",
            label: texts.kind,
            choices: DebtKind::ALL
                .iter()
                .map(|kind| {
                    choice(
                        kind_value(*kind),
                        texts.debt_kind(*kind),
                        filter.kind.as_deref(),
                    )
                })
                .collect(),
        },
        Filter {
            name: "domain",
            label: texts.domain,
            choices: docs
                .domains()
                .iter()
                .map(|d| choice(&d.slug, &d.name, filter.domain.as_deref()))
                .collect(),
        },
    ]
}
