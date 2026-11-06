//! What the rules page shows: every business rule of the run, in its order,
//! kept by the kind, the domain and the feature asked in the query.

use retrodoc_core::model::{BusinessRule, Feature, RuleKind};
use retrodoc_mcp::GeneratedDocs;

use crate::texts::WebTexts;

use crate::view::{active_filters, rule_href, Confidence, Evidence, Filter, Link};

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
    RulesPage {
        filters: filters(docs, filter, texts),
        none_in_run: docs.rules().is_empty(),
        rules,
    }
}

impl RulesPage {
    /// The filters the query gives a value.
    #[must_use]
    pub fn active(&self) -> usize {
        active_filters(&self.filters)
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
