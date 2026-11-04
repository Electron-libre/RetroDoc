//! The history page, its list of runs and the difference of two, asked of the router without a
//! socket over the snapshots a `generate` saves in `.retrodoc/cache/history/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{Actor, ActorKind, ConfidenceScore, Feature, Step, UseCase};
use retrodoc_mcp::history::{
    history_dir, save, Figures, SectionKind, SectionScores, Snapshot, VERSION,
};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use tower::ServiceExt;

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
        confidence: Some(ConfidenceScore::new(0.9, "why".to_string())),
    }
}

fn use_case(slug: &str, feature: &str) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: slug.into(),
        feature_slug: feature.into(),
        name: slug.into(),
        description: String::new(),
        steps: vec![Step {
            order: 1,
            description: "Acts".into(),
            actor: Actor {
                name: "Clerk".into(),
                kind: ActorKind::Human,
            },
            action: String::new(),
            source_refs: Vec::new(),
        }],
        diagram_mermaid: None,
        confidence: None,
        narrative_check: None,
    }
}

/// The docs the interface serves: `billing`, its feature `pay-invoice`, the use cases `refund` and
/// `pay-by-card`.
fn served_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(root, &[feature("pay-invoice", "billing")]).unwrap();
    retrodoc_pipeline::save_use_cases(
        root,
        &[
            use_case("refund", "pay-invoice"),
            use_case("pay-by-card", "pay-invoice"),
        ],
    )
    .unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            ..DomainCluster::default()
        }],
    }
    .save(root)
    .unwrap();
    dir
}

fn section(kind: SectionKind, id: &str, name: &str) -> SectionScores {
    SectionScores {
        kind,
        id: id.into(),
        name: name.into(),
        confidence: Some(0.8),
        business_language: None,
        narrative_check: None,
        set_apart: Vec::new(),
    }
}

fn scored(id: &str, confidence: f32, business: f32, set_apart: &[&str]) -> SectionScores {
    SectionScores {
        confidence: Some(confidence),
        business_language: Some(business),
        set_apart: set_apart.iter().map(|r| (*r).to_string()).collect(),
        ..section(SectionKind::UseCase, id, id.split('/').next_back().unwrap())
    }
}

fn run(day: u8, commit: Option<&str>, sections: Vec<SectionScores>) -> Snapshot {
    Snapshot {
        version: VERSION,
        commit: commit.map(str::to_string),
        finished_at: format!("2026-11-0{day}T10:00:00Z"),
        cost: None,
        figures: Figures {
            confidence: None,
            business_language: None,
            coverage: None,
            debt: Vec::new(),
        },
        sections,
    }
}

/// Run 1 (`20261101T100000Z`), run 2 and run 3 (no commit), oldest first. Run 2 removes the
/// domain `legacy`, the feature `old-feature` and its use case `gone`, adds `cancel`, lowers the
/// confidence of `refund` from 90% to 50%, sets `void` apart and no longer `pay-by-card`. Run 3
/// adds the domain `<script>` (slug `shipping`).
fn with_history(root: &Path) {
    let run_2 = vec![
        section(SectionKind::Domain, "billing", "Billing"),
        section(SectionKind::Feature, "pay-invoice", "pay-invoice"),
        scored("pay-invoice/refund", 0.5, 0.85, &[]),
        scored("pay-invoice/pay-by-card", 0.8, 0.9, &[]),
        scored("pay-invoice/void", 0.8, 0.9, &["business_language"]),
        scored("pay-invoice/cancel", 0.8, 0.9, &[]),
    ];
    let runs = [
        run(
            1,
            Some("aaaaaaa1111"),
            vec![
                section(SectionKind::Domain, "billing", "Billing"),
                section(SectionKind::Domain, "legacy", "Legacy"),
                section(SectionKind::Feature, "pay-invoice", "pay-invoice"),
                section(SectionKind::Feature, "old-feature", "Old feature"),
                scored("pay-invoice/refund", 0.9, 0.9, &[]),
                scored("pay-invoice/pay-by-card", 0.8, 0.9, &["back_office"]),
                scored("pay-invoice/void", 0.8, 0.9, &[]),
                scored("old-feature/gone", 0.8, 0.9, &[]),
            ],
        ),
        run(2, Some("bbbbbbb2222"), run_2.clone()),
        run(
            3,
            None,
            [
                vec![section(SectionKind::Domain, "shipping", "<script>")],
                run_2,
            ]
            .concat(),
        ),
    ];
    for snapshot in &runs {
        assert!(save(root, snapshot).unwrap().is_some());
    }
}

async fn get_in(lang: Option<&'static str>, root: &Path, path: &str) -> String {
    let project = Project {
        lang,
        ..Project::default()
    };
    let response = retrodoc_web::router(
        project,
        GeneratedDocs::load(root, &[]),
        DocLinks::default(),
        Vec::new(),
    )
    .oneshot(Request::get(path).body(Body::empty()).unwrap())
    .await
    .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let page = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

async fn get(root: &Path, path: &str) -> String {
    get_in(None, root, path).await
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{start} not in {text}"));
    let rest = &text[from..];
    let to = rest.find(end).unwrap_or(rest.len());
    &rest[..to]
}

/// The positions of `needles` in `text`, which must hold each.
fn positions(text: &str, needles: &[&str]) -> Vec<usize> {
    needles
        .iter()
        .map(|n| text.find(n).unwrap_or_else(|| panic!("{n} not in {text}")))
        .collect()
}

// spec: HPG-1
#[tokio::test]
async fn the_runs_are_listed_newest_first_with_their_date_and_commit() {
    let repo = served_repo();
    with_history(repo.path());

    let page = get(repo.path(), "/history").await;

    let runs = between(&page, "id=\"history-runs\"", "</section>");
    let at = positions(
        runs,
        &[
            "2026-11-03 10:00:00",
            "2026-11-02 10:00:00",
            "2026-11-01 10:00:00",
        ],
    );
    assert!(at[0] < at[1] && at[1] < at[2], "{runs}");
    assert!(runs.contains("bbbbbbb"), "{runs}");
    assert!(!runs.contains("bbbbbbb2"), "{runs}");
    assert!(runs.contains("no commit"), "{runs}");
}

// spec: HPG-2, HPG-5, HPG-12
#[tokio::test]
async fn by_default_the_newest_run_is_compared_with_the_one_before_it() {
    let repo = served_repo();
    with_history(repo.path());

    let page = get(repo.path(), "/history").await;

    let form = between(&page, "<form class=\"compare\"", "</form>");
    assert!(form.contains("action=\"/history\""), "{form}");
    let from = between(form, "name=\"from\"", "</select>");
    assert!(
        from.contains("value=\"20261102T100000Z\" selected"),
        "{from}"
    );
    assert!(from.contains("value=\"20261101T100000Z\""), "{from}");
    let to = between(form, "name=\"to\"", "</select>");
    assert!(to.contains("value=\"20261103T100000Z\" selected"), "{to}");
    let added = between(&page, "id=\"history-added\"", "</section>");
    assert!(added.contains("&#60;script&#62;"), "{added}");
    assert!(!added.contains("href="), "{added}");
    assert!(!page.contains("id=\"history-removed\""), "{page}");
}

// spec: HPG-3, HPG-6, HPG-7, HPG-8, HPG-10, HPG-11, HPG-12
#[tokio::test]
async fn two_runs_picked_are_compared_section_by_section() {
    let repo = served_repo();
    with_history(repo.path());

    let page = get(
        repo.path(),
        "/history?from=20261101T100000Z&to=20261102T100000Z",
    )
    .await;

    let added = between(&page, "id=\"history-added\"", "</section>");
    assert!(added.contains("cancel"), "{added}");
    assert!(!added.contains("href="), "{added}");
    let removed = between(&page, "id=\"history-removed\"", "</section>");
    let at = positions(removed, &["Legacy", "Old feature", "gone"]);
    assert!(at[0] < at[1] && at[1] < at[2], "{removed}");
    assert!(!removed.contains("billing"), "{removed}");
    let degraded = between(&page, "id=\"history-degraded\"", "</section>");
    assert!(
        degraded.contains("href=\"/domains/billing/pay-invoice/refund\""),
        "{degraded}"
    );
    for figure in ["90%", "50%", "40"] {
        assert!(degraded.contains(figure), "{figure}: {degraded}");
    }
    assert!(!degraded.contains("85%"), "{degraded}");
    let set_apart = between(&page, "id=\"history-set-apart\"", "</section>");
    assert!(set_apart.contains("void"), "{set_apart}");
    assert!(set_apart.contains("business language"), "{set_apart}");
    let freed = between(&page, "id=\"history-no-longer-set-apart\"", "</section>");
    assert!(
        freed.contains("href=\"/domains/billing/pay-invoice/pay-by-card\""),
        "{freed}"
    );
}

// spec: HPG-3, HPG-13
#[tokio::test]
async fn runs_compared_backwards_or_with_themselves_are_compared_as_asked() {
    let repo = served_repo();
    with_history(repo.path());

    let backwards = get(
        repo.path(),
        "/history?from=20261102T100000Z&to=20261101T100000Z",
    )
    .await;
    let same = get(
        repo.path(),
        "/history?from=20261102T100000Z&to=20261102T100000Z",
    )
    .await;

    let removed = between(&backwards, "id=\"history-removed\"", "</section>");
    assert!(removed.contains("cancel"), "{removed}");
    assert!(same.contains("id=\"history-none\""), "{same}");
    assert!(!same.contains("id=\"history-added\""), "{same}");
}

// spec: HPG-4
#[tokio::test]
async fn an_unknown_run_is_replaced_by_its_default() {
    let repo = served_repo();
    with_history(repo.path());

    let page = get(repo.path(), "/history?from=nope&to=20261101T100000Z").await;

    let unknown = between(&page, "id=\"history-unknown\"", "</p>");
    assert!(unknown.contains("nope"), "{unknown}");
    let from = between(&page, "name=\"from\"", "</select>");
    assert!(
        from.contains("value=\"20261102T100000Z\" selected"),
        "{from}"
    );
    let removed = between(&page, "id=\"history-removed\"", "</section>");
    assert!(removed.contains("cancel"), "{removed}");
}

// spec: HPG-14
#[tokio::test]
async fn with_fewer_than_two_runs_the_page_says_the_difference_needs_two() {
    let repo = served_repo();
    let empty = tempfile::tempdir().unwrap();
    let first = run(1, Some("aaaaaaa1111"), Vec::new());
    save(repo.path(), &first).unwrap();

    let one = get(repo.path(), "/history").await;
    let no_run = get(empty.path(), "/history").await;

    assert!(one.contains("2026-11-01 10:00:00"), "{one}");
    for page in [one, no_run] {
        let note = between(&page, "id=\"history-fewer\"", "</p>");
        assert!(note.contains("retrodoc generate"), "{note}");
        assert!(!page.contains("<form class=\"compare\""), "{page}");
    }
    assert!(history_dir(empty.path()).read_dir().is_err());
}

// spec: HPG-15
#[tokio::test]
async fn every_page_links_to_the_history_page_in_the_audit_group() {
    let repo = served_repo();
    let empty = tempfile::tempdir().unwrap();
    for (root, path) in [
        (repo.path(), "/"),
        (repo.path(), "/history"),
        (empty.path(), "/"),
    ] {
        let page = get(root, path).await;
        let nav = between(&page, "aria-label=\"Audit\"", "</nav>");
        let link = between(nav, "href=\"/history\"", "</a>");
        assert!(link.contains(" title=\""), "{path}: {nav}");
    }
}

// spec: WEB-77
#[tokio::test]
async fn the_history_page_is_in_french() {
    let repo = served_repo();
    with_history(repo.path());

    let page = get_in(
        Some("fr"),
        repo.path(),
        "/history?from=20261101T100000Z&to=20261102T100000Z",
    )
    .await;

    for label in ["Historique", "Retirées", "Dégradées", "aucun commit"] {
        assert!(page.contains(label), "{label} missing from {page}");
    }
}
