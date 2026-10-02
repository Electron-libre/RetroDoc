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
    /// Names of the entry points (routes, commands, jobs…) this use case is
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub order: u32,
    pub description: String,
    pub actor: Actor,
    pub action: String,
    /// Files/lines of code this step is grounded on.
    pub source_refs: Vec<SourceRef>,
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
