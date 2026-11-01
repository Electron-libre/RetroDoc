//! The pages of a domain, a feature and a use case, asked of the router
//! without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{
    BusinessRule, ConfidenceScore, Feature, RuleEvidence, RuleKind, SourceRef, UseCase,
};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::{DomainCluster, DomainMap, SourceHashes};
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

/// A run over `billing` (the business feature `pay-invoice` citing
/// `app/invoice.rb` and `app/card.rb`, with a business and a technical use
/// case and a rule; the back-office feature `refund-tool`) and `delivery`
/// (`assign-rider`, no use case), the hashes of the cited files recorded.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("app")).unwrap();
    let mut hashes = SourceHashes::default();
    for name in ["app/invoice.rb", "app/card.rb"] {
        let content = format!("content of {name}");
        std::fs::write(root.join(name), &content).unwrap();
        hashes.put(name, &hash_content(&content));
    }
    hashes.save(root).unwrap();
    let mut pay = feature("pay-invoice", "billing", false);
    pay.source_paths = vec!["app/invoice.rb".into(), "app/card.rb".into()];
    pay.entry_points = vec!["app/invoice.rb#pay".into()];
    retrodoc_pipeline::save_features(
        root,
        &[
            pay,
            feature("refund-tool", "billing", true),
            feature("assign-rider", "delivery", false),
        ],
    )
    .unwrap();
    let mut by_card = use_case("pay-by-card", "pay-invoice");
    by_card.description = "The customer pays by card".into();
    by_card.confidence = Some(ConfidenceScore::new(0.9, None));
    let mut retry = use_case("retry-job", "pay-invoice");
    retry.business_language = Some(ConfidenceScore::new(0.1, None));
    retrodoc_pipeline::save_use_cases(
        root,
        &[by_card, retry, use_case("refund-now", "refund-tool")],
    )
    .unwrap();
    let mut paid = rule("pay-invoice-1", "An invoice is paid within 30 days");
    paid.entities = vec!["Invoice".into()];
    paid.use_cases = vec!["pay-by-card".into()];
    paid.evidence = vec![
        RuleEvidence::Code {
            source: SourceRef {
                path: "app/invoice.rb".into(),
                start_line: Some(4),
                end_line: Some(9),
            },
        },
        RuleEvidence::Test {
            phrase: "refuses a late payment".into(),
            path: "spec/invoice_spec.rb".into(),
        },
    ];
    retrodoc_pipeline::save_rules(root, &[paid]).unwrap();
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
    links.links.insert(
        ItemRef::Feature("pay-invoice".into()),
        vec![DocLink {
            path: "docs/guide.md".into(),
            heading: Some("Paying".into()),
            contradicts: None,
        }],
    );
    links.links.insert(
        ItemRef::Rule("pay-invoice-1".into()),
        vec![DocLink {
            path: "docs/terms.md".into(),
            heading: Some("Delays".into()),
            contradicts: None,
        }],
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

// spec: WEB-16, WEB-18
#[tokio::test]
async fn the_feature_page_shows_its_sections_in_the_order_of_the_markdown_page() {
    let repo = generated_repo();
    let page = page(repo.path(), "/domains/billing/pay-invoice").await;
    assert_in_order(
        main_of(&page),
        &[
            "<h1>Feature pay-invoice</h1>",
            "What pay-invoice does",
            "Confidence",
            "80%",
            "well covered",
            "<h2>Use cases</h2>",
            "href=\"/domains/billing/pay-invoice/pay-by-card\"",
            "90%",
            "The customer pays by card",
            "<h2>Administration and technical</h2>",
            "href=\"/domains/billing/pay-invoice/retry-job\"",
            "<h2>Business rules</h2>",
            "An invoice is paid within 30 days",
            "Limit",
            "30%",
            "Invoice",
            "href=\"/domains/billing/pay-invoice/pay-by-card\"",
            "app/invoice.rb:4-9",
            "refuses a late payment",
            "spec/invoice_spec.rb",
            "href=\"/docs/docs/terms.md#delays\"",
            "<h2>Existing documentation</h2>",
            "href=\"/docs/docs/guide.md#paying\"",
            "<h2>Source files</h2>",
            "app/invoice.rb",
            "app/card.rb",
            "<h2>Entry points</h2>",
            "app/invoice.rb#pay",
        ],
    );
    assert!(!page.contains("badge stale"), "{page}");
    assert!(!main_of(&page).contains("back-office"), "{page}");
}

// spec: WEB-16
#[tokio::test]
async fn a_feature_with_no_use_case_says_none_could_be_derived() {
    let repo = generated_repo();
    let page = page(repo.path(), "/domains/delivery/assign-rider").await;
    assert!(
        main_of(&page).contains("None could be derived from the code."),
        "{page}"
    );
}

// spec: WEB-19
#[tokio::test]
async fn the_feature_page_has_a_breadcrumb_to_its_domain() {
    let repo = generated_repo();
    let page = page(repo.path(), "/domains/billing/pay-invoice").await;
    let start = page.find("<nav class=\"crumbs\"").expect(&page);
    let crumbs = &page[start..start + page[start..].find("</nav>").unwrap()];
    assert_in_order(
        crumbs,
        &[
            "href=\"/\"",
            "href=\"/domains/billing\"",
            "Billing",
            "Feature pay-invoice",
        ],
    );
}

// spec: WEB-20
#[tokio::test]
async fn a_feature_citing_a_file_changed_since_the_run_is_flagged_stale_with_the_file() {
    let repo = generated_repo();
    std::fs::write(repo.path().join("app/card.rb"), "edited since the run").unwrap();
    let page = page(repo.path(), "/domains/billing/pay-invoice").await;
    let start = page.find("class=\"badge stale\"").expect(&page);
    let flag = &page[start..start + page[start..].find("</p>").unwrap()];
    assert!(flag.contains("app/card.rb"), "{flag}");
    assert!(!flag.contains("app/invoice.rb"), "{flag}");
}

// spec: WEB-20
#[tokio::test]
async fn a_cited_file_with_no_recorded_hash_does_not_flag_the_feature() {
    let repo = generated_repo();
    SourceHashes::default().save(repo.path()).unwrap();
    std::fs::write(repo.path().join("app/card.rb"), "edited since the run").unwrap();
    let page = page(repo.path(), "/domains/billing/pay-invoice").await;
    assert!(!page.contains("badge stale"), "{page}");
}

// spec: WEB-21
#[tokio::test]
async fn a_back_office_feature_is_flagged_on_its_page() {
    let repo = generated_repo();
    let page = page(repo.path(), "/domains/billing/refund-tool").await;
    assert!(
        main_of(&page).contains("class=\"badge set-apart\">back-office"),
        "{page}"
    );
    // Every use case of a back-office feature is set apart.
    assert_in_order(
        main_of(&page),
        &[
            "All of them are set apart below.",
            "<h2>Administration and technical</h2>",
            "Use case refund-now",
        ],
    );
}

// spec: WEB-25
#[tokio::test]
async fn a_feature_asked_under_another_domain_or_unknown_is_the_not_found_page() {
    let repo = generated_repo();
    for path in ["/domains/delivery/pay-invoice", "/domains/billing/nope"] {
        let (status, page) = get(repo.path(), path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(page.contains("<h1>Not found</h1>"), "{page}");
    }
}
