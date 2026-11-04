use retrodoc_core::model::{ConfidenceScore, Domain, Feature, UseCase};
use retrodoc_pipeline::usage_log::{record_run, PassRecord, RunUsage};
use retrodoc_pipeline::{Artifact, BoundedContext, ContextMap, Glossary};

use super::*;

fn feature(slug: &str, domain: &str) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: slug.into(),
        description: String::new(),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: None,
    }
}

fn use_case(slug: &str, feature: &str, confidence: f32) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: slug.into(),
        feature_slug: feature.into(),
        name: slug.into(),
        description: String::new(),
        steps: Vec::new(),
        diagram_mermaid: None,
        confidence: Some(ConfidenceScore::new(confidence, None)),
        narrative_check: None,
    }
}

fn docs(use_cases: Vec<UseCase>) -> GeneratedDocs {
    let domains = ["billing", "auth"]
        .map(|slug| Domain {
            slug: slug.into(),
            name: slug.to_uppercase(),
            description: String::new(),
            confidence: None,
        })
        .to_vec();
    let features = vec![feature("pay", "billing"), feature("login", "auth")];
    GeneratedDocs::new(domains, features, use_cases, Glossary::default(), &[])
}

fn run(command: &str, pass: &str) -> RunUsage {
    RunUsage {
        command: command.into(),
        finished_at: "2026-11-04T10:00:00Z".into(),
        wall_ms: 1000,
        passes: vec![PassRecord {
            name: pass.into(),
            wall_ms: 1000,
            models: Vec::new(),
            unparseable: 0,
            skipped: 0,
        }],
    }
}

// spec: AUD-7 (internal: the shade of a cell, which the page only names)
#[test]
fn the_level_of_a_cell_is_its_count_over_the_largest_in_four_steps() {
    assert_eq!(level(0, 8), 0);
    assert_eq!(level(1, 8), 1);
    assert_eq!(level(3, 8), 2);
    assert_eq!(level(8, 8), 4);
}

// spec: AUD-1, AUD-3, AUD-6, AUD-7 (internal: the read model of the audit page, shown by AUD-1)
#[test]
fn the_debt_is_counted_by_domain_and_kind_in_the_order_of_the_run() {
    let docs = docs(vec![
        use_case("refund", "pay", 0.2),
        use_case("void", "pay", 0.3),
        use_case("signin", "login", 0.1),
        use_case("logout", "login", 0.9),
        // Its feature is not in the run: in no row.
        use_case("lost", "gone", 0.1),
    ]);

    let audit = audit(&docs);

    let rows: Vec<(&str, Vec<(usize, u8)>)> = audit
        .heatmap
        .iter()
        .map(|r| {
            (
                r.domain.as_str(),
                r.cells.iter().map(|c| (c.count, c.level)).collect(),
            )
        })
        .collect();
    // A use case low in confidence is also set apart.
    let row = |low: (usize, u8)| {
        let mut cells = vec![(0, 0); DebtKind::WITH_DOMAIN.len()];
        cells[0] = low;
        cells[4] = low;
        cells
    };
    assert_eq!(rows, [("billing", row((2, 4))), ("auth", row((1, 2)))]);
    assert_eq!(audit.heatmap[0].cells[0].kind, DebtKind::LowConfidence);
    assert!(audit.confidence.is_some());
    // `lost` is reached by no feature: the stale check never judges it.
    assert_eq!(audit.stale, Share { count: 0, total: 6 });
}

// spec: AUD-3, AUD-2 (internal: the rounding of a share, which the page only writes)
#[test]
fn a_share_rounds_to_neither_0_nor_100_when_it_is_neither() {
    assert_eq!(
        Share {
            count: 1,
            total: 300
        }
        .percent(),
        1
    );
    assert_eq!(
        Share {
            count: 299,
            total: 300
        }
        .percent(),
        99
    );
    assert_eq!(
        Share {
            count: 0,
            total: 300
        }
        .percent(),
        0
    );
    assert_eq!(
        Share {
            count: 300,
            total: 300
        }
        .percent(),
        100
    );
    assert_eq!(Share { count: 1, total: 4 }.percent(), 25);
    assert_eq!(Share { count: 0, total: 0 }.percent(), 0);
}

// spec: AUD-2, AUD-4, AUD-5 (internal: the figures are read from artifacts only a `generate` with an LLM writes)
#[test]
fn the_figures_are_read_from_the_saved_coverage_usage_and_contexts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let coverage = Artifact::BehaviourCoverage.path(root);
    std::fs::create_dir_all(coverage.parent().unwrap()).unwrap();
    std::fs::write(
        &coverage,
        "by_domain:\n  billing: {entry_points: 4, unplaced: 1}\n  auth: {entry_points: 4, unplaced: 0}\n\
         \x20 gone: {entry_points: 9, unplaced: 9}\n\
         by_feature:\n  pay: {entry_points: 3, uncovered: ['a#x', 'a#y']}\n\
         \x20 gone: {entry_points: 9, uncovered: []}\n",
    )
    .unwrap();
    record_run(root, run("generate", "features")).unwrap();
    record_run(root, run("roles", "roles")).unwrap();
    let context = |slug: &str| BoundedContext {
        slug: slug.into(),
        ..BoundedContext::default()
    };
    ContextMap {
        contexts: vec![context("billing"), context("auth")],
        ..ContextMap::default()
    }
    .save(root)
    .unwrap();
    std::fs::write(
        Artifact::Fingerprints.path(root),
        r#"{"contexts":{"auth":"h"}}"#,
    )
    .unwrap();

    let audit = audit(&docs(Vec::new()).with_repo_root(root));

    let coverage = audit.coverage.unwrap();
    assert_eq!(coverage.placed, Some(Share { count: 7, total: 8 }));
    assert_eq!(coverage.cited, Some(Share { count: 1, total: 3 }));
    assert_eq!(coverage.placed.unwrap().percent(), 88);
    assert_eq!(audit.cost.unwrap().passes[0].name, "features");
    let contexts = audit.contexts.unwrap();
    assert_eq!(contexts.done, ["auth"]);
    assert_eq!(contexts.pending, ["billing"]);
}

// spec: AUD-8 (internal: what the page tells missing, read model side)
#[test]
fn a_run_without_the_saved_figures_has_none_of_them() {
    let dir = tempfile::tempdir().unwrap();

    let audit = audit(&docs(Vec::new()).with_repo_root(dir.path()));

    assert_eq!(audit.confidence, None);
    assert_eq!(audit.business_language, None);
    assert_eq!(audit.coverage, None);
    assert_eq!(audit.cost, None);
    assert_eq!(audit.contexts, None);
    assert_eq!(audit.stale, Share { count: 0, total: 2 });
}
