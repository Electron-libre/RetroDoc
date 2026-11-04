//! What the human reviews change in the debt page and the heatmap of the audit page, asked of the
//! router without a socket over the artifacts a run leaves and the reviews saved in
//! `.retrodoc/reviews.json`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{Actor, ActorKind, ConfidenceScore, Feature, SourceRef, Step, UseCase};
use retrodoc_mcp::reviews::{self, Section, SectionKind, Status};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use tower::ServiceExt;

fn feature(slug: &str, domain: &str, name: &str) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: name.into(),
        description: String::new(),
        source_paths: vec![format!("app/{slug}.rb")],
        entry_points: vec![format!("app/{slug}.rb#create")],
        confidence: Some(ConfidenceScore::new(0.9, "fine".to_string())),
    }
}

fn use_case(slug: &str, feature: &str, name: &str, confidence: Option<f32>) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: slug.into(),
        feature_slug: feature.into(),
        name: name.into(),
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
        confidence: confidence.map(|c| ConfidenceScore::new(c, "few cited lines".to_string())),
        narrative_check: None,
    }
}

fn use_cases() -> Vec<UseCase> {
    vec![
        use_case("refund", "pay-invoice", "Refund", Some(0.3)),
        use_case("pay-by-card", "pay-invoice", "Pay by card", None),
        use_case("ship", "ship-order", "Ship", Some(0.9)),
    ]
}

/// `billing` (feature `pay-invoice`: `Refund` low at 30% and set apart for it, `Pay by card` not
/// scored) and `shipping` (feature `ship-order`, use case `Ship` at 90%); no git history.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(
        root,
        &[
            feature("pay-invoice", "billing", "Pay an invoice"),
            feature("ship-order", "shipping", "Ship an order"),
        ],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(root, &use_cases()).unwrap();
    DomainMap {
        domains: ["billing", "shipping"]
            .map(|slug| DomainCluster {
                slug: slug.into(),
                name: format!("{}{}", slug[..1].to_uppercase(), &slug[1..]),
                ..DomainCluster::default()
            })
            .to_vec(),
    }
    .save(root)
    .unwrap();
    dir
}

fn review(root: &Path, kind: SectionKind, id: &str, status: Status, comment: &str) {
    let docs = GeneratedDocs::load(root, &[]).unwrap();
    reviews::record(root, &docs, &Section::new(kind, id), status, comment).unwrap();
}

async fn get(root: &Path, path: &str) -> String {
    let response = retrodoc_web::router(
        Project::default(),
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

/// The items of the debt list, one string per `<li class="debt-item">`.
fn items(page: &str) -> Vec<String> {
    let start = page.find("id=\"debt-list\"").expect(page);
    page[start..]
        .split("<li class=\"debt-item\"")
        .skip(1)
        .map(str::to_string)
        .collect()
}

/// The kind and the link of each item, in order.
fn listed(page: &str) -> Vec<(String, String)> {
    let field = |item: &str, open: &str, close: &str| {
        item.split_once(open)
            .and_then(|(_, rest)| rest.split_once(close))
            .map_or_else(String::new, |(value, _)| value.to_string())
    };
    items(page)
        .iter()
        .map(|item| {
            (
                field(item, "class=\"badge kind\">", "</span>"),
                field(item, "<a href=\"", "\""),
            )
        })
        .collect()
}

fn pair(kind: &str, href: &str) -> (String, String) {
    (kind.to_string(), href.to_string())
}

// spec: DBT-32
#[tokio::test]
async fn a_validated_section_leaves_the_kinds_of_judgement_only() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/pay-by-card",
        Status::Validated,
        "",
    );
    let page = get(repo.path(), "/debt").await;
    // Still set apart: the docs leave it out by design, whatever a human said.
    assert_eq!(
        listed(&page),
        [pair("Set apart", "/domains/billing/pay-invoice/refund")],
        "{page}"
    );
}

// spec: DBT-32
#[tokio::test]
async fn a_validated_section_rewritten_since_is_listed_again() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    let mut rewritten = use_cases();
    rewritten[0].description = "Refunds the paid invoice.".into();
    retrodoc_pipeline::save_use_cases(repo.path(), &rewritten).unwrap();
    let page = get(repo.path(), "/debt").await;
    assert!(
        listed(&page).contains(&pair(
            "Low confidence",
            "/domains/billing/pay-invoice/refund"
        )),
        "{page}"
    );
}

// spec: DBT-30, DBT-31, DBT-33, DBT-29, DBT-8
#[tokio::test]
async fn a_section_marked_wrong_or_to_review_enters_the_debt_with_its_comment() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::Domain,
        "billing",
        Status::Wrong,
        "Refunds are <b>not</b> billing",
    );
    review(
        repo.path(),
        SectionKind::Feature,
        "ship-order",
        Status::ToReview,
        "",
    );
    review(
        repo.path(),
        SectionKind::UseCase,
        "ship-order/ship",
        Status::Wrong,
        "",
    );
    let page = get(repo.path(), "/debt").await;
    assert_eq!(
        listed(&page),
        [
            // Weight 1 and the entry point of its feature: ahead of the use case, which has none.
            pair("Marked wrong", "/domains/billing"),
            pair("Marked wrong", "/domains/shipping/ship-order/ship"),
            // Weight 0.5 and one entry point, against weight 1 and weakness 0.7.
            pair("To review", "/domains/shipping/ship-order"),
            pair("Low confidence", "/domains/billing/pay-invoice/refund"),
            pair("Not scored", "/domains/billing/pay-invoice/pay-by-card"),
            pair("Set apart", "/domains/billing/pay-invoice/refund"),
        ],
        "{page}"
    );
    let items = items(&page);
    assert!(items[0].contains("Billing"), "{}", items[0]);
    assert!(
        items[0].contains("Refunds are &#60;b&#62;not&#60;/b&#62; billing"),
        "{}",
        items[0]
    );
    assert!(
        items[0].contains("weight 1.00, weakness 1.00, 0 commits, 1 entry point"),
        "{}",
        items[0]
    );
    assert!(items[2].contains("weight 0.50"), "{}", items[2]);
    assert!(!items[2].contains("class=\"why\""), "{}", items[2]);
    let filtered = get(repo.path(), "/debt?kind=to_review&domain=shipping").await;
    assert_eq!(
        listed(&filtered),
        [pair("To review", "/domains/shipping/ship-order")]
    );
}

// spec: DBT-30
#[tokio::test]
async fn a_review_of_a_section_the_run_no_longer_holds_is_not_listed() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::Feature,
        "ship-order",
        Status::Wrong,
        "",
    );
    retrodoc_pipeline::save_features(
        repo.path(),
        &[feature("pay-invoice", "billing", "Pay an invoice")],
    )
    .unwrap();
    let page = get(repo.path(), "/debt").await;
    assert!(!page.contains("Marked wrong</span>"), "{page}");
}

// spec: DBT-34
#[tokio::test]
async fn an_unreadable_file_of_reviews_is_said_and_the_debt_listed_without_it() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    let page = get(repo.path(), "/debt").await;
    assert!(!page.contains("Low confidence</span>"), "{page}");
    std::fs::write(reviews::reviews_path(repo.path()), "{ not json").unwrap();
    let page = get(repo.path(), "/debt").await;
    assert!(page.contains("The reviews could not be read"), "{page}");
    assert!(page.contains("Low confidence</span>"), "{page}");
}

// spec: DBT-34
#[tokio::test]
async fn a_review_saved_while_the_server_runs_shows_on_the_next_debt_page() {
    let repo = generated_repo();
    let app = retrodoc_web::router(
        Project::default(),
        GeneratedDocs::load(repo.path(), &[]),
        DocLinks::default(),
        Vec::new(),
    );
    review(
        repo.path(),
        SectionKind::Feature,
        "ship-order",
        Status::Wrong,
        "",
    );
    let response = app
        .oneshot(Request::get("/debt").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&body).contains("Marked wrong</span>"));
}

// spec: AUD-6
#[tokio::test]
async fn the_heatmap_counts_the_sections_marked_wrong_and_to_review() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::Domain,
        "billing",
        Status::Wrong,
        "",
    );
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::ToReview,
        "",
    );
    let page = get(repo.path(), "/audit").await;
    let heatmap = &page[page.find("id=\"audit-heatmap\"").expect(&page)..];
    let header = heatmap.split("</thead>").next().unwrap();
    let undocumented = header.find("Undocumented").expect(header);
    let wrong = header.find("Marked wrong").expect(header);
    let to_review = header.find("To review").expect(header);
    assert!(undocumented < wrong && wrong < to_review, "{header}");
    assert!(
        heatmap.contains("href=\"/debt?kind=marked_wrong&#38;domain=billing\""),
        "{heatmap}"
    );
    assert!(
        heatmap.contains("href=\"/debt?kind=to_review&#38;domain=billing\""),
        "{heatmap}"
    );
}
