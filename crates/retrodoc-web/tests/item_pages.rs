//! The pages of a domain, a feature and a use case, asked of the router
//! without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{BusinessRule, ConfidenceScore, Feature, RuleKind, UseCase};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use retrodoc_render::{DocLink, DocLinks, ItemRef};
use tower::ServiceExt;

fn feature(slug: &str, domain: &str, back_office: bool) -> Feature {
    Feature {
        back_office,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: format!("Feature {slug}"),
        description: format!("What {slug} does"),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: Some(ConfidenceScore::new(0.8, Some("well covered".into()))),
    }
}

fn use_case(slug: &str, feature: &str) -> UseCase {
    UseCase {
        slug: slug.into(),
        feature_slug: feature.into(),
        name: format!("Use case {slug}"),
        description: String::new(),
        steps: Vec::new(),
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        diagram_mermaid: None,
        confidence: None,
        narrative_check: None,
    }
}

fn rule(slug: &str, statement: &str) -> BusinessRule {
    BusinessRule {
        slug: slug.into(),
        statement: statement.into(),
        kind: RuleKind::Limit,
        entities: Vec::new(),
        use_cases: Vec::new(),
        evidence: Vec::new(),
        confidence: Some(ConfidenceScore::new(0.3, None)),
    }
}

/// A run over `billing` (the business feature `pay-invoice`, the back-office
/// feature `refund-tool`, a rule) and `delivery` (`assign-rider`).
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(
        root,
        &[
            feature("pay-invoice", "billing", false),
            feature("refund-tool", "billing", true),
            feature("assign-rider", "delivery", false),
        ],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(root, &[use_case("pay-by-card", "pay-invoice")]).unwrap();
    retrodoc_pipeline::save_rules(
        root,
        &[rule("pay-invoice-1", "An invoice is paid within 30 days")],
    )
    .unwrap();
    DomainMap {
        domains: vec![
            DomainCluster {
                slug: "billing".into(),
                name: "Billing".into(),
                description: "Invoices and payments".into(),
                ..DomainCluster::default()
            },
            DomainCluster {
                slug: "delivery".into(),
                name: "Delivery".into(),
                description: "Orders brought to customers".into(),
                ..DomainCluster::default()
            },
        ],
    }
    .save(root)
    .unwrap();
    dir
}

/// The guide documents `billing` under "Paying" and states otherwise; the
/// FAQ documents it as a whole.
fn doc_links() -> DocLinks {
    let mut links = DocLinks::default();
    links.links.insert(
        ItemRef::Domain("billing".into()),
        vec![
            DocLink {
                path: "docs/guide.md".into(),
                heading: Some("Paying an invoice".into()),
                contradicts: Some("Invoices are paid in cash".into()),
            },
            DocLink {
                path: "docs/faq.md".into(),
                heading: None,
                contradicts: None,
            },
        ],
    );
    links
}

async fn get(root: &Path, path: &str) -> (StatusCode, String) {
    let docs = GeneratedDocs::load(root, &[]);
    let response = retrodoc_web::router(docs, doc_links())
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn page(root: &Path, path: &str) -> String {
    let (status, body) = get(root, path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

/// The content of the page, without the tree every page shows.
fn main_of(page: &str) -> &str {
    let start = page.find("<main>").expect(page);
    &page[start..page.find("</main>").expect(page)]
}

/// Asserts each of `parts` is in `text`, each after the one before.
fn assert_in_order(text: &str, parts: &[&str]) {
    let mut from = 0;
    for part in parts {
        let at = text[from..]
            .find(part)
            .unwrap_or_else(|| panic!("{part:?} missing after byte {from} of {text}"));
        from += at + part.len();
    }
}

// spec: WEB-15, WEB-18
#[tokio::test]
async fn the_domain_page_shows_its_sections_in_the_order_of_the_markdown_page() {
    let repo = generated_repo();
    let page = page(repo.path(), "/domains/billing").await;
    assert_in_order(
        main_of(&page),
        &[
            "<h1>Billing</h1>",
            "Invoices and payments",
            "Confidence",
            "<h2>Features</h2>",
            "href=\"/domains/billing/pay-invoice\"",
            "Feature pay-invoice",
            "80%",
            "well covered",
            "What pay-invoice does",
            "<h2>Administration and technical</h2>",
            "href=\"/domains/billing/refund-tool\"",
            "<h2>Business rules</h2>",
            "An invoice is paid within 30 days",
            "Limit",
            "class=\"badge low\"",
            "30%",
            "href=\"/domains/billing/pay-invoice\"",
            "<h2>Existing documentation</h2>",
        ],
    );
    assert!(!main_of(&page).contains("Feature assign-rider"), "{page}");
}

// spec: WEB-15
#[tokio::test]
async fn a_domain_with_no_back_office_feature_has_no_technical_section() {
    let repo = generated_repo();
    let page = page(repo.path(), "/domains/delivery").await;
    assert!(page.contains("Feature assign-rider"), "{page}");
    assert!(!page.contains("Administration and technical"), "{page}");
    assert!(!page.contains("<h2>Business rules</h2>"), "{page}");
    assert!(!page.contains("<h2>Existing documentation</h2>"), "{page}");
}

// spec: WEB-19
#[tokio::test]
async fn the_domain_page_has_a_breadcrumb_to_the_home_page() {
    let repo = generated_repo();
    let page = page(repo.path(), "/domains/billing").await;
    let start = page.find("<nav class=\"crumbs\"").expect(&page);
    let crumbs = &page[start..start + page[start..].find("</nav>").unwrap()];
    assert_in_order(crumbs, &["href=\"/\"", "Billing"]);
}

// spec: WEB-23
#[tokio::test]
async fn the_existing_docs_are_links_to_their_section_with_what_they_state_otherwise() {
    let repo = generated_repo();
    let page = page(repo.path(), "/domains/billing").await;
    let docs = &page[page.find("<h2>Existing documentation</h2>").expect(&page)..];
    assert_in_order(
        docs,
        &[
            "href=\"/docs/docs/guide.md#paying-an-invoice\"",
            "docs/guide.md",
            "Paying an invoice",
            "states otherwise",
            "Invoices are paid in cash",
            "href=\"/docs/docs/faq.md\"",
        ],
    );
}

// spec: WEB-25
#[tokio::test]
async fn an_unknown_domain_is_the_not_found_page() {
    let repo = generated_repo();
    let (status, page) = get(repo.path(), "/domains/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(page.contains("<h1>Not found</h1>"), "{page}");
}
