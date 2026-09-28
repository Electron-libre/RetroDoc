//! Domain model for the functional doc (see PLAN.md §1-3).
//!
//! These types are the shared vocabulary between the `retrodoc-pipeline`
//! crate (which produces them) and `retrodoc-render` (which serializes them
//! to Markdown). They are not populated yet in phase 1 (foundation):
//! clustering and generation arrive in later roadmap phases.

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

/// Stable, human-readable identifier (slug) used for file names and
/// cross-document links.
pub type Slug = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub slug: Slug,
    pub name: String,
    pub description: String,
    pub sub_domains: Vec<SubDomain>,
    pub confidence: ConfidenceScore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubDomain {
    pub slug: Slug,
    pub name: String,
    pub description: String,
    pub confidence: ConfidenceScore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub slug: Slug,
    pub domain_slug: Slug,
    pub sub_domain_slug: Option<Slug>,
    pub name: String,
    pub description: String,
    pub confidence: ConfidenceScore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseCase {
    pub slug: Slug,
    pub feature_slug: Slug,
    pub name: String,
    pub description: String,
    pub steps: Vec<Step>,
    /// Mermaid diagram (flowchart/sequenceDiagram) illustrating the process.
    pub diagram_mermaid: Option<String>,
    pub confidence: ConfidenceScore,
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
