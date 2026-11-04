use retrodoc_core::model::{ConfidenceScore, Domain, Feature, UseCase};
use retrodoc_pipeline::usage_log::{ModelRecord, PassRecord, RunUsage};
use retrodoc_pipeline::{Artifact, Glossary};

use super::*;
use crate::audit::Share;
use crate::read_model::Uncovered;

fn feature(slug: &str, domain: &str, back_office: bool) -> Feature {
    Feature {
        back_office,
        technical: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: slug.to_uppercase(),
        description: String::new(),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: Some(ConfidenceScore::new(0.8, None)),
    }
}

fn use_case(slug: &str, feature: &str, confidence: f32) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: Some(ConfidenceScore::new(0.9, None)),
        slug: slug.into(),
        feature_slug: feature.into(),
        name: slug.to_uppercase(),
        description: String::new(),
        steps: Vec::new(),
        diagram_mermaid: None,
        confidence: Some(ConfidenceScore::new(confidence, None)),
        narrative_check: Some(ConfidenceScore::new(1.0, None)),
    }
}

fn docs(use_cases: Vec<UseCase>) -> GeneratedDocs {
    let domains = ["billing", "auth"]
        .map(|slug| Domain {
            slug: slug.into(),
            name: slug.to_uppercase(),
            description: String::new(),
            confidence: Some(ConfidenceScore::new(0.7, None)),
        })
        .to_vec();
    let features = vec![
        feature("pay", "billing", false),
        feature("admin", "auth", true),
    ];
    GeneratedDocs::new(domains, features, use_cases, Glossary::default(), &[])
}

fn sample() -> GeneratedDocs {
    docs(vec![
        use_case("refund", "pay", 0.2),
        use_case("void", "pay", 0.9),
        use_case("ban", "admin", 0.9),
    ])
}

fn usage(finished_at: &str) -> RunUsage {
    let model = |calls, prompt| ModelRecord {
        model: "m".into(),
        calls,
        calls_without_usage: 0,
        prompt_tokens: prompt,
        completion_tokens: 10,
        cached_tokens: 5,
    };
    let pass = |name: &str, models| PassRecord {
        name: name.into(),
        wall_ms: 500,
        models,
        unparseable: 0,
        skipped: 0,
    };
    RunUsage {
        command: "generate".into(),
        finished_at: finished_at.into(),
        wall_ms: 1200,
        passes: vec![
            pass("features", vec![model(2, 100)]),
            pass("confidence", vec![model(3, 50), model(1, 7)]),
        ],
    }
}

const END: &str = "2026-11-04T10:00:00.123+00:00";

fn files(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(history_dir(root))
        .map(|dir| {
            dir.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

// spec: RHI-2, RHI-3, RHI-6 (internal: the content of the snapshot; through the binary a fake run per figure)
#[test]
fn the_snapshot_holds_the_commit_date_scores_and_cost_of_the_run() {
    let snapshot = snapshot(&sample(), Some("abc123".into()), END, Some(&usage(END)));

    assert_eq!(snapshot.version, VERSION);
    assert_eq!(snapshot.commit.as_deref(), Some("abc123"));
    assert_eq!(snapshot.finished_at, END);
    let report = docs_report(&sample());
    assert_eq!(snapshot.figures.confidence, report.overall);
    assert_eq!(snapshot.figures.business_language, report.overall_business);
    assert!(snapshot.figures.confidence.is_some());
    assert_eq!(
        snapshot.cost,
        Some(Cost {
            wall_ms: 1200,
            calls: 6,
            prompt_tokens: 157,
            cached_tokens: 15,
            completion_tokens: 30,
        })
    );
}

// spec: RHI-2, RHI-3, RHI-4, RHI-6 (internal: the content of the snapshot; through the binary a fake run per figure)
#[test]
fn a_figure_the_run_lacks_is_absent() {
    let mut bare = use_case("refund", "pay", 0.5);
    bare.confidence = None;
    bare.business_language = None;
    let dir = tempfile::tempdir().unwrap();

    let snapshot = snapshot(
        &docs(vec![bare]).with_repo_root(dir.path()),
        None,
        END,
        None,
    );

    assert_eq!(snapshot.commit, None);
    assert_eq!(snapshot.figures.confidence, None);
    assert_eq!(snapshot.figures.business_language, None);
    assert_eq!(snapshot.figures.coverage, None);
    assert_eq!(snapshot.cost, None);
}

// spec: RHI-4 (internal: the coverage is read from an artifact only a `generate` with an LLM writes)
#[test]
fn the_coverage_is_the_one_of_the_audit_page() {
    let dir = tempfile::tempdir().unwrap();
    let coverage = Artifact::BehaviourCoverage.path(dir.path());
    std::fs::create_dir_all(coverage.parent().unwrap()).unwrap();
    std::fs::write(
        &coverage,
        "by_domain:\n  billing: {entry_points: 4, unplaced: 1}\n\
         by_feature:\n  pay: {entry_points: 3, uncovered: ['a#x']}\n",
    )
    .unwrap();

    let snapshot = snapshot(&sample().with_repo_root(dir.path()), None, END, None);

    let coverage = snapshot.figures.coverage.unwrap();
    assert_eq!(coverage.placed, Some(Share { count: 3, total: 4 }));
    assert_eq!(coverage.cited, Some(Share { count: 2, total: 3 }));
}

// spec: RHI-5 (internal: the counts of the debt by kind and domain, from fixture docs)
#[test]
fn the_debt_is_counted_by_kind_and_domain_without_the_stale_items() {
    let docs = sample().with_uncovered(Uncovered {
        no_domain: vec!["lib/a.rb".into(), "lib/b.rb".into(), "c.rb".into()],
        ..Uncovered::default()
    });

    let snapshot = snapshot(&docs, None, END, None);

    let counts: Vec<(&str, Option<&str>, usize)> = snapshot
        .figures
        .debt
        .iter()
        .map(|c| (c.kind.as_str(), c.domain.as_deref(), c.count))
        .collect();
    // `refund` is low in confidence, hence set apart; `ban` is set apart by its back-office feature.
    assert_eq!(
        counts,
        [
            ("low_confidence", Some("billing"), 1),
            ("set_apart", Some("auth"), 1),
            ("set_apart", Some("billing"), 1),
            ("no_domain", None, 2),
        ]
    );
    assert!(snapshot.figures.debt.iter().all(|c| c.kind != "stale"));
}

// spec: RHI-7, RHI-8 (internal: the sections of the snapshot, from fixture docs)
#[test]
fn every_domain_feature_and_use_case_is_listed_in_the_order_of_the_run() {
    let snapshot = snapshot(&sample(), None, END, None);

    let ids: Vec<(SectionKind, &str, &str)> = snapshot
        .sections
        .iter()
        .map(|s| (s.kind, s.id.as_str(), s.name.as_str()))
        .collect();
    assert_eq!(
        ids,
        [
            (SectionKind::Domain, "billing", "BILLING"),
            (SectionKind::Domain, "auth", "AUTH"),
            (SectionKind::Feature, "pay", "PAY"),
            (SectionKind::Feature, "admin", "ADMIN"),
            (SectionKind::UseCase, "pay/refund", "REFUND"),
            (SectionKind::UseCase, "pay/void", "VOID"),
            (SectionKind::UseCase, "admin/ban", "BAN"),
        ]
    );
    let refund = &snapshot.sections[4];
    assert_eq!(refund.confidence, Some(0.2));
    assert_eq!(refund.business_language, Some(0.9));
    assert_eq!(refund.narrative_check, Some(1.0));
    assert_eq!(refund.set_apart, ["confidence"]);
    assert_eq!(snapshot.sections[6].set_apart, ["back_office"]);
    let domain = &snapshot.sections[0];
    assert_eq!(domain.confidence, Some(0.7));
    assert_eq!(domain.business_language, None);
    assert_eq!(domain.set_apart, Vec::<String>::new());
}

// spec: RHI-9, RHI-13 (internal: the store of the snapshots; through the binary in tests/run_history.rs)
#[test]
fn a_snapshot_equal_to_the_last_but_for_commit_date_and_cost_is_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let first = save(root, &snapshot(&sample(), Some("a".into()), END, None)).unwrap();
    let rerun = snapshot(
        &sample(),
        Some("b".into()),
        "2026-11-05T09:00:00Z",
        Some(&usage(END)),
    );
    let second = save(root, &rerun).unwrap();
    let changed = docs(vec![use_case("refund", "pay", 0.3)]);
    let third = save(
        root,
        &snapshot(&changed, Some("b".into()), "2026-11-06T09:00:00Z", None),
    )
    .unwrap();

    assert!(first.is_some());
    assert_eq!(second, None);
    assert!(third.is_some());
    let saved = load(root);
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[0].commit.as_deref(), Some("a"));
    assert_eq!(saved[1].finished_at, "2026-11-06T09:00:00Z");
    assert_eq!(saved[1].sections[4].confidence, Some(0.3));
}

// spec: RHI-1, RHI-11 (internal: two runs in the same second, which a test through the binary cannot time)
#[test]
fn a_snapshot_named_like_a_saved_one_gets_a_name_of_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    save(root, &snapshot(&sample(), None, END, None)).unwrap();
    let changed = docs(vec![use_case("refund", "pay", 0.3)]);
    save(root, &snapshot(&changed, None, END, None)).unwrap();

    assert_eq!(
        files(root),
        ["20261104T100000Z.json", "20261104T100000Z_2.json"]
    );
    let saved = load(root);
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[1].sections[4].confidence, Some(0.3));
}

// spec: RHI-13 (internal: a damaged file, which no command writes)
#[test]
fn a_file_that_cannot_be_read_is_left_out_of_the_list() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    save(root, &snapshot(&sample(), None, END, None)).unwrap();
    std::fs::write(
        history_dir(root).join("20261103T000000Z.json"),
        "{ not json",
    )
    .unwrap();

    let saved = load(root);

    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].finished_at, END);
    assert_eq!(load(&root.join("missing")), Vec::new());
}

// spec: RHI-9 (internal: a damaged newest file, which no command writes)
#[test]
fn a_snapshot_is_compared_with_the_newest_file_even_when_it_cannot_be_read() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    save(root, &snapshot(&sample(), None, END, None)).unwrap();
    std::fs::write(history_dir(root).join("20261105T000000Z.json"), "{ cut").unwrap();

    let rerun = save(
        root,
        &snapshot(&sample(), None, "2026-11-06T00:00:00Z", None),
    )
    .unwrap();

    assert!(rerun.is_some());
    assert_eq!(load(root).len(), 2);
}

// spec: RHI-1, RHI-13 (internal: a file of a later format, which this version never writes)
#[test]
fn a_snapshot_of_a_later_format_is_left_out_and_no_draft_is_left_behind() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut later = snapshot(&sample(), None, "2026-11-03T00:00:00Z", None);
    later.version = VERSION + 1;
    std::fs::create_dir_all(history_dir(root)).unwrap();
    std::fs::write(
        history_dir(root).join("20261103T000000Z.json"),
        serde_json::to_string(&later).unwrap(),
    )
    .unwrap();

    save(root, &snapshot(&sample(), None, END, None)).unwrap();

    assert_eq!(load(root).len(), 1);
    assert_eq!(load(root)[0].version, VERSION);
    assert_eq!(
        files(root),
        ["20261103T000000Z.json", "20261104T100000Z.json"]
    );
}
