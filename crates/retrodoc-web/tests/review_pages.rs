//! The reviews of the sections on their pages and in the lists of their parents, asked of the
//! router without a socket over the artifacts a run leaves in `.retrodoc/cache/` and the reviews
//! of `.retrodoc/reviews.json`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
use retrodoc_mcp::reviews::{self, Section, SectionKind, Status};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use tower::ServiceExt;

fn feature(slug: &str) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: "billing".into(),
        name: format!("Feature {slug}"),
        description: format!("What {slug} does"),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: Some(ConfidenceScore::new(0.8, None)),
    }
}

fn use_case(slug: &str) -> UseCase {
    UseCase {
        slug: slug.into(),
        feature_slug: "pay-invoice".into(),
        name: format!("Use case {slug}"),
        description: String::new(),
        steps: Vec::new(),
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        diagram_mermaid: None,
        confidence: Some(ConfidenceScore::new(0.9, None)),
        narrative_check: None,
    }
}

/// A run over `billing`: the features `pay-invoice` (the use cases `pay-by-card` and
/// `pay-by-cash`) and `refund`.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(root, &[feature("pay-invoice"), feature("refund")]).unwrap();
    retrodoc_pipeline::save_use_cases(root, &[use_case("pay-by-card"), use_case("pay-by-cash")])
        .unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            description: "Invoices and payments".into(),
            ..DomainCluster::default()
        }],
    }
    .save(root)
    .unwrap();
    dir
}

fn load(root: &Path) -> GeneratedDocs {
    GeneratedDocs::load(root, &[]).unwrap()
}

fn record(root: &Path, kind: SectionKind, id: &str, status: Status, comment: &str) {
    reviews::record(root, &load(root), &Section::new(kind, id), status, comment).unwrap();
}

async fn get(root: &Path, path: &str) -> (StatusCode, String) {
    get_in(root, path, Some("en")).await
}

async fn get_in(root: &Path, path: &str, lang: Option<&'static str>) -> (StatusCode, String) {
    let project = Project {
        name: "Shop".into(),
        lang,
        ..Project::default()
    };
    let response = retrodoc_web::router(project, Some(load(root)), DocLinks::default(), Vec::new())
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// The review block of a section page.
fn review_block(page: &str) -> &str {
    let start = page
        .find("<section class=\"review\"")
        .unwrap_or_else(|| panic!("no review block in {page}"));
    let end = start + page[start..].find("</section>").unwrap();
    &page[start..end]
}

/// The item of a list of the content that links to `href`: a row of the list of a page, else the card of a domain
/// on the home page.
fn row<'a>(page: &'a str, href: &str) -> &'a str {
    // After the tree of every page, which links to the same pages.
    let page = &page[page.find("<main").unwrap()..];
    let (start, close) = if let Some(at) = page.find(&format!("<li><a href=\"{href}\">")) {
        (at, "</li>")
    } else {
        let at = page
            .find(&format!("<h3><a href=\"{href}\">"))
            .unwrap_or_else(|| panic!("no row of {href} in {page}"));
        (page[..at].rfind("<article").unwrap(), "</article>")
    };
    let end = start + page[start..].find(close).unwrap();
    &page[start..end]
}

// spec: WEB-79
#[tokio::test]
async fn a_section_page_shows_its_review_after_its_confidence() {
    let dir = generated_repo();
    let root = dir.path();
    std::fs::write(
        reviews::reviews_path(root),
        r#"{"version": 1, "reviews": [{"kind": "feature", "id": "pay-invoice", "status": "wrong",
            "comment": "Misses the <b>refunds</b>", "author": "Ada",
            "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "x"}]}"#,
    )
    .unwrap();

    let (status, page) = get(root, "/domains/billing/pay-invoice").await;

    assert_eq!(status, StatusCode::OK);
    let block = review_block(&page);
    assert!(
        block.contains("class=\"badge review wrong\">Wrong</span>"),
        "{block}"
    );
    assert!(
        block.contains("Misses the &#60;b&#62;refunds&#60;/b&#62;"),
        "{block}"
    );
    assert!(block.contains("Ada"), "{block}");
    assert!(block.contains("2026-11-04T10:00:00Z"), "{block}");
    let confidence = page.find("class=\"score\"").unwrap();
    assert!(confidence < page.find("<section class=\"review\"").unwrap());
}

// spec: WEB-79
#[tokio::test]
async fn a_review_with_no_author_shows_its_status_comment_and_date() {
    let dir = generated_repo();
    let root = dir.path();
    // Not a git repository: no `user.name`.
    record(
        root,
        SectionKind::UseCase,
        "pay-invoice/pay-by-card",
        Status::Validated,
        "",
    );

    let (_, page) = get(root, "/domains/billing/pay-invoice/pay-by-card").await;

    let block = review_block(&page);
    assert!(block.contains(">Validated</span>"), "{block}");
    assert!(!block.contains("by "), "{block}");
    let saved = reviews::load(root).unwrap().reviews()[0]
        .reviewed_at
        .clone();
    assert!(block.contains(saved.as_str()), "{block}");
}

// spec: WEB-79
#[tokio::test]
async fn a_section_with_no_review_says_it_is_not_reviewed() {
    let dir = generated_repo();
    for path in [
        "/domains/billing",
        "/domains/billing/refund",
        "/domains/billing/pay-invoice/pay-by-cash",
    ] {
        let (_, page) = get(dir.path(), path).await;
        assert!(review_block(&page).contains("Not reviewed"), "{path}");
    }
}

// spec: WEB-80
#[tokio::test]
async fn the_rows_of_the_lists_carry_the_status_of_their_review() {
    let dir = generated_repo();
    let root = dir.path();
    record(root, SectionKind::Domain, "billing", Status::ToReview, "");
    record(root, SectionKind::Feature, "pay-invoice", Status::Wrong, "");
    record(
        root,
        SectionKind::UseCase,
        "pay-invoice/pay-by-card",
        Status::Validated,
        "",
    );

    let (_, home) = get(root, "/").await;
    let billing = row(&home, "/domains/billing");
    assert!(
        billing.contains("badge review to_review\">To review<"),
        "{billing}"
    );

    let (_, domain) = get(root, "/domains/billing").await;
    let pay = row(&domain, "/domains/billing/pay-invoice");
    assert!(pay.contains("badge review wrong\">Wrong<"), "{pay}");
    assert!(!row(&domain, "/domains/billing/refund").contains("badge review"));

    let (_, feature) = get(root, "/domains/billing/pay-invoice").await;
    let card = row(&feature, "/domains/billing/pay-invoice/pay-by-card");
    assert!(
        card.contains("badge review validated\">Validated<"),
        "{card}"
    );
    assert!(!row(&feature, "/domains/billing/pay-invoice/pay-by-cash").contains("badge review"));
}

// spec: WEB-82
#[tokio::test]
async fn a_review_saved_while_the_server_runs_shows_on_the_next_request() {
    let dir = generated_repo();
    let root = dir.path();
    let router = retrodoc_web::router(
        Project::default(),
        Some(load(root)),
        DocLinks::default(),
        Vec::new(),
    );
    let ask = |router: axum::Router| async move {
        let response = router
            .oneshot(
                Request::get("/domains/billing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    };
    assert!(review_block(&ask(router.clone()).await).contains("Not reviewed"));

    record(
        root,
        SectionKind::Domain,
        "billing",
        Status::Validated,
        "Checked",
    );

    let page = ask(router).await;
    assert!(review_block(&page).contains("Checked"), "{page}");
}

// spec: WEB-87
#[tokio::test]
async fn an_unreadable_reviews_file_is_named_in_place_of_the_review() {
    let dir = generated_repo();
    let root = dir.path();
    std::fs::write(reviews::reviews_path(root), "{ not json").unwrap();

    for path in [
        "/",
        "/domains/billing",
        "/domains/billing/pay-invoice",
        "/domains/billing/pay-invoice/pay-by-card",
    ] {
        let (status, page) = get(root, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        if path != "/" {
            let block = review_block(&page);
            assert!(block.contains("reviews.json cannot be read"), "{block}");
        }
    }
}

// spec: WEB-87
#[tokio::test]
async fn a_reviews_file_of_a_later_format_is_named_with_its_version() {
    let dir = generated_repo();
    let root = dir.path();
    std::fs::write(
        reviews::reviews_path(root),
        r#"{"version": 99, "reviews": []}"#,
    )
    .unwrap();

    let (_, page) = get(root, "/domains/billing").await;

    assert!(review_block(&page).contains("of format 99"), "{page}");
}

// spec: WEB-77, WEB-79
#[tokio::test]
async fn the_review_is_worded_in_french() {
    let dir = generated_repo();
    let root = dir.path();
    record(
        root,
        SectionKind::Feature,
        "pay-invoice",
        Status::ToReview,
        "",
    );

    let (_, page) = get_in(root, "/domains/billing/pay-invoice", Some("fr")).await;
    assert!(review_block(&page).contains(">À revoir</span>"), "{page}");
    let (_, page) = get_in(root, "/domains/billing/refund", Some("fr")).await;
    assert!(review_block(&page).contains("Non relu"), "{page}");
}
