//! What the rules page shows: every business rule of the run, in its order,
//! kept by the kind, the domain and the feature asked in the query.

use retrodoc_core::model::{BusinessRule, Feature, RuleKind};
use retrodoc_mcp::GeneratedDocs;

use crate::texts::WebTexts;

use crate::view::{rule_href, Confidence, Evidence, Link};

/// The kinds, in the order of the form.
const KINDS: [RuleKind; 5] = [
    RuleKind::Validation,
    RuleKind::Permission,
    RuleKind::StateTransition,
    RuleKind::Limit,
    RuleKind::Computation,
];

/// The value of `kind` in the query: its name in the artifacts.
fn kind_value(kind: RuleKind) -> &'static str {
    match kind {
        RuleKind::Validation => "validation",
        RuleKind::Permission => "permission",
        RuleKind::StateTransition => "state_transition",
        RuleKind::Limit => "limit",
        RuleKind::Computation => "computation",
    }
}

/// What the query asks: an empty value asks nothing.
#[derive(Default)]
pub struct RuleFilter {
    pub kind: Option<String>,
    pub domain: Option<String>,
    pub feature: Option<String>,
}

pub struct RulesPage {
    /// The kind, the domain and the feature, in that order.
    pub filters: Vec<Filter>,
    /// The filters the query gives a value.
    pub active: usize,
    /// The run has no rule at all, whatever the filters.
    pub none_in_run: bool,
    pub rules: Vec<RuleRow>,
}

/// A filter of the form: a group of chips (radio choices) or a select, "All" first.
pub struct Filter {
    /// The name of its query parameter.
    pub name: &'static str,
    pub label: &'static str,
    /// Radio choices rather than a select: for the few kinds, not the domains nor features.
    pub chips: bool,
    /// The query gives it no value: "All" is the choice.
    pub all: bool,
    pub choices: Vec<Choice>,
}

impl Filter {
    /// A filter of `name` whose choices are `(value, label)`, the one `asked` selected.
    pub fn new<'a>(
        name: &'static str,
        label: &'static str,
        chips: bool,
        asked: Option<&str>,
        choices: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Self {
        Self {
            name,
            label,
            chips,
            all: asked.is_none(),
            choices: choices
                .into_iter()
                .map(|(value, label)| Choice {
                    value: value.to_string(),
                    label: label.to_string(),
                    selected: asked == Some(value),
                })
                .collect(),
        }
    }
}

/// How many of `filters` the query gives a value.
#[must_use]
pub fn active_filters(filters: &[Filter]) -> usize {
    filters.iter().filter(|filter| !filter.all).count()
}

pub struct Choice {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

pub struct RuleRow {
    pub statement: String,
    pub kind: RuleKind,
    pub confidence: Confidence,
    /// Its item on the page of its feature, labelled with the feature and its
    /// domain; `None` when its feature is not in the run.
    pub feature: Option<Link>,
    pub evidence: Vec<Evidence>,
}

/// The rules page of `docs` for `filter`.
#[must_use]
pub fn rules_page(docs: &GeneratedDocs, filter: &RuleFilter, texts: &WebTexts) -> RulesPage {
    let rules = docs
        .rules()
        .iter()
        .map(|rule| (rule, feature_of(docs, rule)))
        .filter(|(rule, feature)| filter.keeps(rule, *feature))
        .map(|(rule, feature)| RuleRow {
            statement: rule.statement.clone(),
            kind: rule.kind,
            confidence: Confidence::of(rule.confidence.as_ref()),
            feature: feature.map(|f| Link {
                href: rule_href(f, &rule.slug),
                name: feature_label(docs, f),
            }),
            evidence: rule.evidence.iter().map(Evidence::of).collect(),
        })
        .collect();
    let filters = filters(docs, filter, texts);
    RulesPage {
        active: active_filters(&filters),
        filters,
        none_in_run: docs.rules().is_empty(),
        rules,
    }
}

fn feature_of<'a>(docs: &'a GeneratedDocs, rule: &BusinessRule) -> Option<&'a Feature> {
    docs.features()
        .iter()
        .find(|f| rule.belongs_to_feature(&f.slug))
}

/// `<feature> (<domain>)`: two domains may have features of the same name.
fn feature_label(docs: &GeneratedDocs, feature: &Feature) -> String {
    let domain = docs
        .domains()
        .iter()
        .find(|d| d.slug == feature.domain_slug)
        .map_or(feature.domain_slug.as_str(), |d| d.name.as_str());
    format!("{} ({domain})", feature.name)
}

impl RuleFilter {
    fn keeps(&self, rule: &BusinessRule, feature: Option<&Feature>) -> bool {
        let kind = self
            .kind
            .as_deref()
            .is_none_or(|asked| kind_value(rule.kind) == asked);
        let domain = self
            .domain
            .as_deref()
            .is_none_or(|asked| feature.is_some_and(|f| f.domain_slug == asked));
        let feature = self
            .feature
            .as_deref()
            .is_none_or(|asked| feature.is_some_and(|f| f.slug == asked));
        kind && domain && feature
    }
}

fn filters(docs: &GeneratedDocs, filter: &RuleFilter, texts: &WebTexts) -> Vec<Filter> {
    let features: Vec<(&str, String)> = docs
        .features()
        .iter()
        .map(|f| (f.slug.as_str(), feature_label(docs, f)))
        .collect();
    vec![
        Filter::new(
            "kind",
            texts.kind,
            true,
            filter.kind.as_deref(),
            KINDS
                .iter()
                .map(|kind| (kind_value(*kind), texts.doc.kind(*kind))),
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
        Filter::new(
            "feature",
            texts.feature,
            false,
            filter.feature.as_deref(),
            features.iter().map(|(slug, label)| (*slug, label.as_str())),
        ),
    ]
}
