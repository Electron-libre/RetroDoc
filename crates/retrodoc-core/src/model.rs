//! Domain model for the functional doc (see PLAN.md §1-3).
//!
//! These types are the shared vocabulary between the `retrodoc-pipeline`
//! crate (which produces them) and `retrodoc-render` (which serializes them
//! to Markdown). Populated progressively by the roadmap phases:
//! features and use cases since phase 4, confidence scores from phase 5.

use serde::{Deserialize, Serialize};

/// Confidence score attached to a generated section, aggregated in the
/// coverage report (`_retrodoc/coverage-report.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceScore {
    /// Between 0.0 (no confidence) and 1.0 (claim verified against the cited code).
    pub value: f32,
    /// Short explanation: why this score (e.g. "no code found for this step").
    pub rationale: Option<String>,
}

impl ConfidenceScore {
    pub fn new(value: f32, rationale: impl Into<Option<String>>) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            rationale: rationale.into(),
        }
    }
}

/// Sections scoring below this are flagged as documentation debt, in the
/// coverage report and in the rendered docs.
pub const LOW_CONFIDENCE_THRESHOLD: f32 = 0.5;

/// A use case that reads below this business-language score is set apart in the docs
/// (`UseCase::is_technical`).
pub const MIN_BUSINESS_LANGUAGE: f32 = 0.8;

/// A use case below this confidence is set apart in the docs (`UseCase::is_technical`).
pub const MIN_USE_CASE_CONFIDENCE: f32 = 0.6;

/// Stable, human-readable identifier (slug) used for file names and
/// cross-document links.
pub type Slug = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub slug: Slug,
    pub name: String,
    pub description: String,
    pub sub_domains: Vec<SubDomain>,
    /// `None` until the confidence pass (roadmap phase 5) has scored it.
    #[serde(default)]
    pub confidence: Option<ConfidenceScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubDomain {
    pub slug: Slug,
    pub name: String,
    pub description: String,
    /// `None` until the confidence pass (roadmap phase 5) has scored it.
    #[serde(default)]
    pub confidence: Option<ConfidenceScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub slug: Slug,
    pub domain_slug: Slug,
    pub sub_domain_slug: Option<Slug>,
    pub name: String,
    pub description: String,
    /// Source files (repo-relative) this feature is grounded on; the code
    /// its use cases are derived from.
    #[serde(default)]
    pub source_paths: Vec<String>,
    /// Entry points (`<file>#<name>`) this feature groups, when the features come from the
    /// surface (`domains_from = "surface"`); each has one use case. Empty for features built
    /// from files, and absent from old `features.yaml`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entry_points: Vec<String>,
    /// A back-office or administration tool, as the LLM judged it from the roles of the
    /// feature's files and its description. Its use cases are set apart in the docs.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub back_office: bool,
    /// `None` until the confidence pass (roadmap phase 5) has scored it.
    #[serde(default)]
    pub confidence: Option<ConfidenceScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseCase {
    pub slug: Slug,
    pub feature_slug: Slug,
    pub name: String,
    pub description: String,
    pub steps: Vec<Step>,
    /// Ids (`<file>#<name>`; names only in artifacts from before) of the entry points (routes, commands, jobs…) this use case is
    /// triggered by, copied from the entry points inventory. Empty when the
    /// feature has no known entry point.
    #[serde(default)]
    pub entry_points: Vec<String>,
    /// The business actor who triggers the use case, as named in the actors
    /// list (`actors.yaml`). `None` when no actor is known.
    #[serde(default)]
    pub primary_actor: Option<String>,
    /// Business-level account of the use case: who does what and why, in the
    /// application's own vocabulary, without code-level detail. The steps
    /// below are the technical level. `None` on artifacts from before this
    /// level existed.
    #[serde(default)]
    pub narrative: Option<String>,
    /// How business-level the use case reads (0.0 = code talk, 1.0 = business
    /// language), next to `confidence`, which says how well the code supports
    /// it. `None` until the vocabulary pass has scored it.
    #[serde(default)]
    pub business_language: Option<ConfidenceScore>,
    /// Mermaid diagram (flowchart/sequenceDiagram) illustrating the process.
    pub diagram_mermaid: Option<String>,
    /// `None` until the confidence pass (roadmap phase 5) has scored it.
    #[serde(default)]
    pub confidence: Option<ConfidenceScore>,
}

impl UseCase {
    /// The names of the entry points it covers, without the file of their id.
    #[must_use]
    pub fn entry_point_names(&self) -> Vec<&str> {
        self.entry_points
            .iter()
            .map(|id| match id.split_once('#') {
                // A file has a dot or a slash; `Class#method`, a name saved before ids, has neither.
                Some((file, name)) if file.contains(['.', '/']) => name,
                _ => id.as_str(),
            })
            .collect()
    }

    /// Whether the docs set this use case apart from the business ones: it belongs to a
    /// back-office feature, or reads under the business-language or confidence threshold (a
    /// score not yet computed is not a reason).
    #[must_use]
    pub fn is_technical(&self, feature_back_office: bool) -> bool {
        feature_back_office
            || self
                .business_language
                .as_ref()
                .is_some_and(|s| s.value < MIN_BUSINESS_LANGUAGE)
            || self
                .confidence
                .as_ref()
                .is_some_and(|s| s.value < MIN_USE_CASE_CONFIDENCE)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub order: u32,
    pub description: String,
    pub actor: Actor,
    /// A short verb phrase. Empty when the LLM wrote one text per step, kept
    /// in `description` (`issues/shorter_use_case_answers.md`).
    #[serde(default)]
    pub action: String,
    /// Files/lines of code this step is grounded on.
    pub source_refs: Vec<SourceRef>,
}

impl Step {
    /// What the step does in few words: its action, else its text.
    #[must_use]
    pub fn label(&self) -> &str {
        if self.action.trim().is_empty() {
            self.description.trim()
        } else {
            &self.action
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Actor {
    pub name: String,
    pub kind: ActorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    Human,
    System,
}

/// Reference to a code location, used for grounding and cross-checking
/// (confidence pass, PLAN.md §2 step 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRef {
    pub path: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
}

/// What a business rule constrains (ADR 0021).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// A value or an input that is accepted or refused.
    Validation,
    /// Who may or may not do something.
    Permission,
    /// The states an object can move through, and when.
    StateTransition,
    /// A bound: a maximum, a minimum, a quota, a deadline.
    Limit,
    /// How an amount, a choice or a date is worked out.
    Computation,
}

/// What backs a rule: code, a test description or a doc section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleEvidence {
    Code {
        #[serde(flatten)]
        source: SourceRef,
    },
    /// A test description that states the behaviour, with the file it comes from.
    Test { phrase: String, path: String },
    /// A section of an existing doc, with its file and heading.
    Doc { path: String, heading: String },
}

/// A rule of the business — what is allowed, refused, limited, and in which
/// states — stated in business language with its evidence (ADR 0021).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusinessRule {
    /// Stable id (`<feature-slug>-<n>`), reused by renderers and the MCP server.
    pub slug: Slug,
    /// The rule in the application's own vocabulary.
    pub statement: String,
    pub kind: RuleKind,
    /// Glossary entities the rule applies to.
    #[serde(default)]
    pub entities: Vec<String>,
    /// Slugs of the use cases the rule applies to.
    #[serde(default)]
    pub use_cases: Vec<Slug>,
    #[serde(default)]
    pub evidence: Vec<RuleEvidence>,
    /// `None` until the rules pass has scored it.
    #[serde(default)]
    pub confidence: Option<ConfidenceScore>,
}

impl BusinessRule {
    /// Whether this is one of the rules of the feature `feature_slug`: the slug is
    /// `<feature-slug>-<number>`.
    #[must_use]
    pub fn belongs_to_feature(&self, feature_slug: &str) -> bool {
        self.number_in(feature_slug).is_some()
    }

    /// The number of the rule among those of the feature `feature_slug`, `None` when it is not
    /// one of them.
    #[must_use]
    pub fn number_in(&self, feature_slug: &str) -> Option<usize> {
        self.slug
            .strip_prefix(feature_slug)
            .and_then(|rest| rest.strip_prefix('-'))
            .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            .and_then(|n| n.parse().ok())
    }

    /// Whether a test phrase or a doc section backs the rule next to the code.
    #[must_use]
    pub fn has_corroboration(&self) -> bool {
        self.evidence
            .iter()
            .any(|e| matches!(e, RuleEvidence::Test { .. } | RuleEvidence::Doc { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn use_case(business_language: Option<f32>, confidence: Option<f32>) -> UseCase {
        UseCase {
            slug: "u".into(),
            feature_slug: "f".into(),
            name: "U".into(),
            description: String::new(),
            steps: Vec::new(),
            entry_points: Vec::new(),
            primary_actor: None,
            narrative: None,
            business_language: business_language.map(|v| ConfidenceScore::new(v, None)),
            diagram_mermaid: None,
            confidence: confidence.map(|v| ConfidenceScore::new(v, None)),
        }
    }

    #[test]
    fn a_step_with_one_text_has_no_action_and_is_labelled_by_its_text() {
        let step: Step = serde_json::from_str(
            r#"{"order":1,"description":"Saves the invoice.","actor":{"name":"System","kind":"system"},"source_refs":[]}"#,
        )
        .unwrap();
        assert_eq!(step.action, "");
        assert_eq!(step.label(), "Saves the invoice.");
        let step = Step {
            action: "saves".into(),
            ..step
        };
        assert_eq!(step.label(), "saves");
    }

    #[test]
    fn a_use_case_is_technical_in_a_back_office_feature_or_under_a_threshold() {
        assert!(use_case(Some(0.9), Some(0.9)).is_technical(true));
        assert!(use_case(Some(0.79), Some(0.9)).is_technical(false));
        assert!(use_case(Some(0.9), Some(0.59)).is_technical(false));
        assert!(!use_case(Some(0.8), Some(0.6)).is_technical(false));
        // Not scored yet is no reason to set it apart.
        assert!(!use_case(None, None).is_technical(false));
    }

    fn rule() -> BusinessRule {
        BusinessRule {
            slug: "orders-1".into(),
            statement: "An order whose delivery would take an hour or more is refused.".into(),
            kind: RuleKind::Limit,
            entities: vec!["Order".into()],
            use_cases: vec!["place-an-order".into()],
            evidence: vec![RuleEvidence::Code {
                source: SourceRef {
                    path: "lib/router.rb".into(),
                    start_line: Some(12),
                    end_line: Some(18),
                },
            }],
            confidence: Some(ConfidenceScore::new(0.6, None)),
        }
    }

    #[test]
    fn rule_round_trips_through_json() {
        let mut r = rule();
        r.evidence.push(RuleEvidence::Test {
            phrase: "refuses an order that takes 60 minutes".into(),
            path: "spec/router_spec.rb".into(),
        });
        r.evidence.push(RuleEvidence::Doc {
            path: "README.md".into(),
            heading: "Limits".into(),
        });
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<BusinessRule>(&json).unwrap(), r);
    }

    #[test]
    fn rule_reads_with_only_the_required_fields() {
        let r: BusinessRule =
            serde_json::from_str(r#"{"slug":"a-1","statement":"s","kind":"state_transition"}"#)
                .unwrap();
        assert_eq!(r.kind, RuleKind::StateTransition);
        assert!(r.evidence.is_empty() && r.confidence.is_none());
    }

    #[test]
    fn a_rule_belongs_to_the_feature_whose_slug_and_number_it_carries() {
        let mut r = rule();
        r.slug = "orders-12".into();
        assert!(r.belongs_to_feature("orders"));
        r.slug = "orders-new-1".into();
        assert!(!r.belongs_to_feature("orders"));
        r.slug = "orders-".into();
        assert!(!r.belongs_to_feature("orders"));
        r.slug = "orders".into();
        assert!(!r.belongs_to_feature("orders"));
    }

    #[test]
    fn only_tests_and_docs_corroborate() {
        let mut r = rule();
        assert!(!r.has_corroboration());
        r.evidence.push(RuleEvidence::Doc {
            path: "README.md".into(),
            heading: "Limits".into(),
        });
        assert!(r.has_corroboration());
    }
}
