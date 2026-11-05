//! The audit page, its figures and its heatmap, asked of the router without a socket over the
//! artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{Actor, ActorKind, ConfidenceScore, Feature, SourceRef, Step, UseCase};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::usage_log::{record_run, ModelRecord, PassRecord, RunUsage};
use retrodoc_pipeline::{
    Artifact, BoundedContext, ContextMap, DomainCluster, DomainMap, SourceHashes,
};
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use tower::ServiceExt;

fn score(value: f32) -> ConfidenceScore {
    ConfidenceScore::new(value, "why".to_string())
}

fn feature(slug: &str, domain: &str) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: slug.into(),
        description: String::new(),
        source_paths: vec![format!("app/{slug}.rb")],
        entry_points: Vec::new(),
        confidence: Some(score(0.9)),
    }
}

fn use_case(slug: &str, feature: &str, confidence: Option<f32>) -> UseCase {
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
            source_refs: vec![SourceRef {
                path: format!("app/{feature}.rb"),
                start_line: Some(1),
                end_line: None,
            }],
        }],
        diagram_mermaid: None,
        confidence: confidence.map(score),
        narrative_check: None,
    }
}

/// `billing` (`refund` low at 30%, so also set apart; `pay-by-card` not scored) and `<script>`
/// (slug `shipping`, whose feature file changed since the run: the feature and `ship` stale).
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(
        root,
        &[
            feature("pay-invoice", "billing"),
            feature("ship-order", "shipping"),
        ],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(
        root,
        &[
            use_case("refund", "pay-invoice", Some(0.3)),
            use_case("pay-by-card", "pay-invoice", None),
            use_case("ship", "ship-order", Some(0.9)),
        ],
    )
    .unwrap();
    DomainMap {
        domains: [("billing", "Billing"), ("shipping", "<script>")]
            .map(|(slug, name)| DomainCluster {
                slug: slug.into(),
                name: name.into(),
                ..DomainCluster::default()
            })
            .to_vec(),
    }
    .save(root)
    .unwrap();
    std::fs::create_dir_all(root.join("app")).unwrap();
    let mut hashes = SourceHashes::default();
    for name in ["app/pay-invoice.rb", "app/ship-order.rb"] {
        std::fs::write(root.join(name), name).unwrap();
        hashes.put(name, &hash_content(name));
    }
    hashes.save(root).unwrap();
    std::fs::write(root.join("app/ship-order.rb"), "edited").unwrap();
    dir
}

/// The coverage, usage and contexts a `generate` by bounded contexts saves.
fn with_saved_figures(root: &Path) {
    let coverage = Artifact::BehaviourCoverage.path(root);
    std::fs::write(
        coverage,
        "by_domain:\n  billing: {entry_points: 4, unplaced: 1}\n\
         by_feature:\n  pay-invoice: {entry_points: 4, uncovered: ['a#x']}\n",
    )
    .unwrap();
    let pass = |name: &str, calls: u64, without: u64| PassRecord {
        name: name.into(),
        wall_ms: 75_000,
        models: vec![ModelRecord {
            model: "deepseek-chat".into(),
            calls,
            calls_without_usage: without,
            prompt_tokens: 12_400,
            completion_tokens: 900,
            cached_tokens: 3_000,
        }],
        unparseable: 0,
        skipped: 0,
    };
    let run = |command: &str, day: u8, passes| RunUsage {
        command: command.into(),
        finished_at: format!("2026-11-0{day}T10:00:00Z"),
        wall_ms: 150_000,
        passes,
    };
    let passes = vec![pass("features", 3, 0), pass("use_cases", 2, 1)];
    record_run(root, run("generate", 2, passes)).unwrap();
    record_run(root, run("roles", 3, vec![pass("roles", 1, 0)])).unwrap();
    let context = |slug: &str| BoundedContext {
        slug: slug.into(),
        ..BoundedContext::default()
    };
    ContextMap {
        contexts: vec![context("billing"), context("shipping"), context("returns")],
        ..ContextMap::default()
    }
    .save(root)
    .unwrap();
    std::fs::write(
        Artifact::Fingerprints.path(root),
        r#"{"contexts":{"billing":"h"}}"#,
    )
    .unwrap();
}

async fn get_in(lang: Option<&'static str>, root: &Path, path: &str) -> (StatusCode, String) {
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
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn audit(root: &Path) -> String {
    let (status, page) = get_in(None, root, "/audit").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{start} not in {text}"));
    let rest = &text[from..];
    let to = rest.find(end).unwrap_or(rest.len());
    &rest[..to]
}

/// The text of an HTML fragment, its tags and the end of its opening tag left out.
fn text_of(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = true;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.trim().to_string()
}

/// The cells of the row of `domain`, each `(level, text)`.
fn row(page: &str, domain: &str) -> Vec<(String, String)> {
    let table = between(page, "<table class=\"heatmap\"", "</table>");
    let row = between(table, &format!("<th scope=\"row\">{domain}</th>"), "</tr>");
    row.split("<td class=\"level-")
        .skip(1)
        .map(|cell| {
            let (level, rest) = cell.split_once('"').unwrap();
            let content = &rest[..rest.find("</td>").unwrap()];
            (level.to_string(), text_of(content))
        })
        .collect()
}

// spec: AUD-6, AUD-7, WEB-10
#[tokio::test]
async fn the_heatmap_counts_the_debt_of_each_domain_by_kind_and_links_to_the_debt_view() {
    let repo = generated_repo();

    let page = audit(repo.path()).await;

    let table = between(&page, "<table class=\"heatmap\"", "</table>");
    for kind in [
        "Low confidence",
        "Not scored",
        "Set apart",
        "Stale",
        "Contradicted",
        "Undocumented",
    ] {
        assert!(table.contains(kind), "{kind} not a column of {table}");
    }
    assert!(!table.contains("No domain"), "{table}");
    let cell = |level: &str, text: &str| (level.to_string(), text.to_string());
    assert_eq!(
        row(&page, "Billing"),
        [
            cell("2", "1"),
            cell("2", "1"),
            cell("0", ""),
            cell("0", ""),
            cell("2", "1"),
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
        ]
    );
    assert_eq!(
        row(&page, "&#60;script&#62;"),
        [
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
            cell("4", "2"),
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
            cell("0", ""),
        ]
    );
    let stale = between(table, "<td class=\"level-4\"", "</td>");
    assert!(
        stale.contains("href=\"/debt?kind=stale&#38;domain=shipping\""),
        "{stale}"
    );
    let empty = between(table, "<td class=\"level-0\"", "</td>");
    assert!(!empty.contains("href"), "{empty}");
}

// spec: AUD-1, AUD-3, AUD-8
#[tokio::test]
async fn the_figures_the_run_has_are_shown_and_the_missing_ones_say_how_to_get_them() {
    let repo = generated_repo();

    let page = audit(repo.path()).await;

    let confidence = between(&page, "id=\"audit-confidence\"", "</section>");
    // The mean of 30% and 90%.
    assert!(
        confidence.contains("<span class=\"badge\">60%</span>"),
        "{confidence}"
    );
    let stale = between(&page, "id=\"audit-stale\"", "</section>");
    assert!(stale.contains("2 of 5"), "{stale}");
    assert!(stale.contains("40%"), "{stale}");
    for id in ["audit-coverage", "audit-cost", "audit-contexts"] {
        let section = between(&page, &format!("id=\"{id}\""), "</section>");
        assert!(section.contains("retrodoc generate"), "{id}: {section}");
    }
}

// spec: AUD-1
#[tokio::test]
async fn an_overall_confidence_below_the_threshold_is_flagged_low() {
    let repo = generated_repo();
    retrodoc_pipeline::save_use_cases(repo.path(), &[use_case("ship", "ship-order", Some(0.2))])
        .unwrap();

    let page = audit(repo.path()).await;

    let confidence = between(&page, "id=\"audit-confidence\"", "</section>");
    assert!(
        confidence.contains("<span class=\"badge low\">20%</span>"),
        "{confidence}"
    );
}

// spec: AUD-1, AUD-8
#[tokio::test]
async fn a_run_with_no_confidence_says_it_is_missing_and_still_shows_the_heatmap() {
    let repo = generated_repo();
    let mut ship = use_case("ship", "ship-order", None);
    ship.business_language = Some(score(0.9));
    retrodoc_pipeline::save_use_cases(repo.path(), &[ship]).unwrap();

    let page = audit(repo.path()).await;

    let confidence = between(&page, "id=\"audit-confidence\"", "</section>");
    assert!(confidence.contains("retrodoc generate"), "{confidence}");
    assert!(confidence.contains("Business language"), "{confidence}");
    assert!(page.contains("<table class=\"heatmap\""), "{page}");
}

// spec: AUD-2, AUD-4, AUD-5
#[tokio::test]
async fn the_coverage_the_cost_of_the_last_generate_and_the_contexts_are_shown() {
    let repo = generated_repo();
    with_saved_figures(repo.path());

    let page = audit(repo.path()).await;

    let coverage = between(&page, "id=\"audit-coverage\"", "</section>");
    assert!(coverage.contains("3 of 4"), "{coverage}");
    assert!(coverage.contains("75%"), "{coverage}");
    let cost = between(&page, "id=\"audit-cost\"", "</section>");
    assert!(cost.contains("2026-11-02"), "{cost}");
    assert!(cost.contains(">features<"), "{cost}");
    assert!(cost.contains("1m15s"), "{cost}");
    assert!(cost.contains("12,400"), "{cost}");
    assert!(cost.contains("24,800"), "{cost}");
    assert!(!cost.contains(">roles<"), "{cost}");
    assert!(cost.contains("lower bound"), "{cost}");
    let contexts = between(&page, "id=\"audit-contexts\"", "</section>");
    assert!(contexts.contains("1 of 3"), "{contexts}");
    assert!(contexts.contains("&#60;script&#62;"), "{contexts}");
    assert!(contexts.contains("returns"), "{contexts}");
}

// spec: AUD-9, AUD-10
#[tokio::test]
async fn every_page_links_to_the_audit_page_and_with_no_run_it_says_to_run_generate() {
    let repo = generated_repo();
    let empty = tempfile::tempdir().unwrap();
    for (root, path) in [
        (repo.path(), "/"),
        (repo.path(), "/debt"),
        (empty.path(), "/"),
    ] {
        let (_, page) = get_in(None, root, path).await;
        let nav = between(&page, "aria-label=\"Audit\"", "</nav>");
        let link = between(nav, "href=\"/audit\"", "</a>");
        assert!(link.contains(" title=\""), "{path}: {nav}");
    }
    let (_, home) = get_in(None, repo.path(), "/").await;
    let start = between(&home, "id=\"start-here\"", "</section>");
    assert!(!start.contains("/audit"), "{start}");
    let page = audit(empty.path()).await;
    assert!(page.contains("retrodoc generate"), "{page}");
}

// spec: WEB-77, AUD-6
#[tokio::test]
async fn the_audit_page_is_in_french() {
    let repo = generated_repo();
    let (_, page) = get_in(Some("fr"), repo.path(), "/audit").await;
    for label in ["Confiance faible", "Périmé", "Tableau de bord"] {
        assert!(page.contains(label), "{label} missing from {page}");
    }
}
