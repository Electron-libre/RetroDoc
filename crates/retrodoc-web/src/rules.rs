//! What the rules page shows: every business rule of the run, in its order,
//! kept by the kind, the domain and the feature asked in the query.

use retrodoc_core::model::{BusinessRule, Feature, RuleKind};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_render::Texts;

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
    /// The run has no rule at all, whatever the filters.
    pub none_in_run: bool,
    pub rules: Vec<RuleRow>,
}

/// A select of the form.
pub struct Filter {
    /// The name of its query parameter.
    pub name: &'static str,
    pub label: &'static str,
    pub choices: Vec<Choice>,
}

pub struct Choice {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

pub struct RuleRow {
    pub statement: String,
    pub kind: &'static str,
    pub confidence: Confidence,
    /// Its item on the page of its feature, labelled with the feature and its
    /// domain; `None` when its feature is not in the run.
    pub feature: Option<Link>,
    pub evidence: Vec<Evidence>,
}

/// The rules page of `docs` for `filter`.
#[must_use]
pub fn rules_page(docs: &GeneratedDocs, filter: &RuleFilter) -> RulesPage {
    let rules = docs
        .rules()
        .iter()
        .map(|rule| (rule, feature_of(docs, rule)))
        .filter(|(rule, feature)| filter.keeps(rule, *feature))
        .map(|(rule, feature)| RuleRow {
            statement: rule.statement.clone(),
            kind: Texts::ENGLISH.kind(rule.kind),
            confidence: Confidence::of(rule.confidence.as_ref()),
            feature: feature.map(|f| Link {
                href: rule_href(f, &rule.slug),
                name: feature_label(docs, f),
            }),
            evidence: rule.evidence.iter().map(Evidence::of).collect(),
        })
        .collect();
    RulesPage {
        filters: filters(docs, filter),
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

fn filters(docs: &GeneratedDocs, filter: &RuleFilter) -> Vec<Filter> {
    let choice = |value: &str, label: &str, asked: Option<&str>| Choice {
        value: value.to_string(),
        label: label.to_string(),
        selected: asked == Some(value),
    };
    vec![
        Filter {
            name: "kind",
            label: "Kind",
            choices: KINDS
                .iter()
                .map(|kind| {
                    choice(
                        kind_value(*kind),
                        Texts::ENGLISH.kind(*kind),
                        filter.kind.as_deref(),
                    )
                })
                .collect(),
        },
        Filter {
            name: "domain",
            label: "Domain",
            choices: docs
                .domains()
                .iter()
                .map(|d| choice(&d.slug, &d.name, filter.domain.as_deref()))
                .collect(),
        },
        Filter {
            name: "feature",
            label: "Feature",
            choices: docs
                .features()
                .iter()
                .map(|f| {
                    let label = feature_label(docs, f);
                    choice(&f.slug, &label, filter.feature.as_deref())
                })
                .collect(),
        },
    ]
}
