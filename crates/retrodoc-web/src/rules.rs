//! What the rules page shows: every business rule of the run, in its order,
//! kept by the kind, the domain and the feature asked in the query.

use retrodoc_core::model::{BusinessRule, Feature, RuleKind};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_render::kind_label;

use crate::view::{feature_href, rule_anchor, Confidence, Evidence, Link};

/// The kinds, each with its value in the query (its name in the artifacts).
const KINDS: [(RuleKind, &str); 5] = [
    (RuleKind::Validation, "validation"),
    (RuleKind::Permission, "permission"),
    (RuleKind::StateTransition, "state_transition"),
    (RuleKind::Limit, "limit"),
    (RuleKind::Computation, "computation"),
];

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
            kind: kind_label(rule.kind),
            confidence: Confidence::of(rule.confidence.as_ref()),
            feature: feature.map(|f| Link {
                href: format!("{}#{}", feature_href(f), rule_anchor(&rule.slug)),
                name: format!("{} ({})", f.name, domain_name(docs, &f.domain_slug)),
            }),
            evidence: rule.evidence.iter().map(Evidence::of).collect(),
        })
        .collect();
    RulesPage {
        filters: filters(docs, filter),
        rules,
    }
}

fn feature_of<'a>(docs: &'a GeneratedDocs, rule: &BusinessRule) -> Option<&'a Feature> {
    docs.features()
        .iter()
        .find(|f| rule.belongs_to_feature(&f.slug))
}

fn domain_name<'a>(docs: &'a GeneratedDocs, slug: &'a str) -> &'a str {
    docs.domains()
        .iter()
        .find(|d| d.slug == slug)
        .map_or(slug, |d| d.name.as_str())
}

impl RuleFilter {
    fn keeps(&self, rule: &BusinessRule, feature: Option<&Feature>) -> bool {
        let kind = self.kind.as_deref().is_none_or(|asked| {
            KINDS
                .iter()
                .any(|(kind, value)| *kind == rule.kind && *value == asked)
        });
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
                .map(|(kind, value)| choice(value, kind_label(*kind), filter.kind.as_deref()))
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
                .map(|f| choice(&f.slug, &f.name, filter.feature.as_deref()))
                .collect(),
        },
    ]
}
