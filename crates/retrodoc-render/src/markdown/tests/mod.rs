use std::path::Path;

use retrodoc_core::model::{
    Actor, ActorKind, ConfidenceScore, Domain, Feature, SourceRef, Step, UseCase,
};

use crate::{RenderedFile, RunMetadata, AGENT_INDEX_PATH};

mod agent_index;
mod pages;

pub(super) fn meta() -> RunMetadata {
    RunMetadata {
        model: "m".to_string(),
        commit: Some("abc".to_string()),
        generated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

pub(super) fn sample() -> (Vec<Domain>, Vec<Feature>, Vec<UseCase>) {
    let domains = vec![Domain {
        slug: "billing".to_string(),
        name: "Billing".to_string(),
        description: "Invoices.".to_string(),
        confidence: Some(ConfidenceScore::new(0.8, None)),
    }];
    let features = vec![
        Feature {
            back_office: false,
            slug: "pay-invoice".to_string(),
            domain_slug: "billing".to_string(),
            name: "Pay an invoice".to_string(),
            description: "Customers pay.".to_string(),
            source_paths: vec!["src/pay.rs".to_string()],
            entry_points: Vec::new(),
            confidence: Some(ConfidenceScore::new(0.3, "weak".to_string())),
        },
        Feature {
            back_office: false,
            slug: "list".to_string(),
            domain_slug: "billing".to_string(),
            name: "List".to_string(),
            description: "Listing.".to_string(),
            source_paths: vec![],
            entry_points: Vec::new(),
            confidence: None,
        },
    ];
    let use_cases = vec![UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: "pay-by-card".to_string(),
        feature_slug: "pay-invoice".to_string(),
        name: "Pay by card".to_string(),
        description: "Card flow.".to_string(),
        steps: vec![Step {
            order: 1,
            description: "Submit the form".to_string(),
            actor: Actor {
                name: "Customer".to_string(),
                kind: ActorKind::Human,
            },
            action: "submit".to_string(),
            source_refs: vec![SourceRef {
                path: "src/pay.rs".to_string(),
                start_line: Some(3),
                end_line: Some(9),
            }],
        }],
        diagram_mermaid: Some("sequenceDiagram\n    A->>B: x".to_string()),
        confidence: Some(ConfidenceScore::new(0.3, "step 1: guessed".to_string())),
    }];
    (domains, features, use_cases)
}

pub(super) fn page<'a>(files: &'a [RenderedFile], path: &str) -> &'a str {
    &files
        .iter()
        .find(|f| f.path == Path::new(path))
        .unwrap()
        .content
}

fn index_of(files: &[RenderedFile]) -> &str {
    &files
        .iter()
        .find(|f| f.path == Path::new(AGENT_INDEX_PATH))
        .unwrap()
        .content
}
