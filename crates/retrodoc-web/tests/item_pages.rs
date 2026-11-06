//! The pages of a domain, a feature and a use case, asked of the router
//! without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{
    Actor, ActorKind, BusinessRule, ConfidenceScore, Feature, RuleEvidence, RuleKind, SourceRef,
    Step, UseCase,
};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::{DomainCluster, DomainMap, SourceHashes};
use retrodoc_render::{DocLink, DocLinks, ItemRef};
use tower::ServiceExt;

fn feature(slug: &str, domain: &str, back_office: bool) -> Feature {
    Feature {
        back_office,
        technical: false,
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

fn source(path: &str, start_line: Option<u32>, end_line: Option<u32>) -> SourceRef {
    SourceRef {
        path: path.into(),
        start_line,
        end_line,
    }
}

fn step(
    order: u32,
    actor: &str,
    kind: ActorKind,
    action: &str,
    description: &str,
    source_refs: &[SourceRef],
) -> Step {
    Step {
        order,
        description: description.into(),
        actor: Actor {
            name: actor.into(),
            kind,
        },
        action: action.into(),
        source_refs: source_refs.to_vec(),
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
        confidence: Some(ConfidenceScore::new(
            0.3,
            Some("backed by code alone".into()),
        )),
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
    by_card.business_language = Some(ConfidenceScore::new(
        0.85,
        Some("reads as the business".into()),
    ));
    by_card.primary_actor = Some("Customer".into());
    by_card.entry_points = vec!["app/invoice.rb#pay".into()];
    by_card.narrative = Some("The customer pays the invoice with a card.".into());
    by_card.steps = vec![
        step(
            1,
            "Customer",
            ActorKind::Human,
            "Submits the card",
            "Fills the card form",
            &[
                source("app/invoice.rb", Some(4), Some(9)),
                source("app/card.rb", Some(2), None),
            ],
        ),
        step(
            2,
            "Gateway",
            ActorKind::System,
            "",
            "Charges the card",
            &[source("app/card.rb", None, None)],
        ),
    ];
    by_card.diagram_mermaid = Some("sequenceDiagram\n  Customer->>Gateway: pay".into());
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
        ItemRef::UseCase {
            feature: "pay-invoice".into(),
            use_case: "pay-by-card".into(),
        },
        vec![DocLink {
            path: "docs/card.md".into(),
            heading: None,
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
    get_in(root, path, Some("en")).await
}

/// [`get`] with the pages in the language of the tag `lang`.
async fn get_in(root: &Path, path: &str, lang: Option<&'static str>) -> (StatusCode, String) {
    let docs = GeneratedDocs::load(root, &[]);
    let response = retrodoc_web::router(
        retrodoc_web::Project {
            lang,
            ..retrodoc_web::Project::default()
        },
        docs,
        doc_links(),
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

/// The previous / next links of a use case page, empty when it has none.
fn sequence(page: &str) -> &str {
    page.find("<nav class=\"sequence\"").map_or("", |start| {
        &page[start..start + page[start..].find("</nav>").unwrap()]
    })
}

// spec: WEB-68
#[tokio::test]
async fn a_use_case_links_to_the_previous_and_next_use_cases_of_its_feature() {
    let repo = generated_repo();
    let first = page(repo.path(), PAY_BY_CARD).await;
    let links = sequence(&first);
    assert!(!links.contains("Previous"), "{links}");
    assert_in_order(
        links,
        &[
            "href=\"/domains/billing/pay-invoice/retry-job\"",
            "Use case retry-job",
        ],
    );
    let last = page(repo.path(), "/domains/billing/pay-invoice/retry-job").await;
    let links = sequence(&last);
    assert!(!links.contains("Next"), "{links}");
    assert_in_order(
        links,
        &["href=\"", "/pay-by-card\"", "Use case pay-by-card"],
    );
}

// spec: WEB-68
#[tokio::test]
async fn the_single_use_case_of_a_feature_has_neither_previous_nor_next() {
    let repo = generated_repo();
    let page = page(repo.path(), "/domains/billing/refund-tool/refund-now").await;
    assert_eq!(sequence(&page), "", "{page}");
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

/// Replaces the use cases of the run with `use_cases`.
fn save_use_cases(root: &Path, use_cases: &[UseCase]) {
    retrodoc_pipeline::save_use_cases(root, use_cases).unwrap();
}

fn saved_use_case(root: &Path, slug: &str) -> UseCase {
    retrodoc_pipeline::load_use_cases(root)
        .unwrap()
        .into_iter()
        .find(|u| u.slug == slug)
        .unwrap()
}

const PAY_BY_CARD: &str = "/domains/billing/pay-invoice/pay-by-card";

// spec: WEB-17, WEB-18
#[tokio::test]
async fn the_use_case_page_shows_its_sections_in_the_order_of_the_markdown_page() {
    let repo = generated_repo();
    let page = page(repo.path(), PAY_BY_CARD).await;
    assert_in_order(
        main_of(&page),
        &[
            "<h1>Use case pay-by-card</h1>",
            "The customer pays by card",
            "Confidence",
            "90%",
            "Business language",
            "85%",
            "reads as the business",
            "Primary actor",
            "Customer",
            "Triggered by",
            "<code>pay</code>",
            "<h2>What happens</h2>",
            "The customer pays the invoice with a card.",
            "<h2>Business rules</h2>",
            "An invoice is paid within 30 days",
            "Steps",
            "Customer",
            "human",
            "Submits the card",
            "Fills the card form",
            "app/invoice.rb:4-9",
            "app/card.rb:2",
            "Gateway",
            "system",
            "Charges the card",
            "app/card.rb",
            "Diagram",
            "Customer-&#62;&#62;Gateway: pay",
            "<h2>Existing documentation</h2>",
            "href=\"/docs/docs/card.md\"",
        ],
    );
    assert!(!page.contains("badge stale"), "{page}");
    assert!(!page.contains("unbacked"), "{page}");
}

// spec: WEB-17
#[tokio::test]
async fn a_use_case_with_no_narrative_shows_its_steps_without_a_narrative_section() {
    let repo = generated_repo();
    let mut by_card = saved_use_case(repo.path(), "pay-by-card");
    by_card.narrative = None;
    by_card.business_language = None;
    save_use_cases(repo.path(), &[by_card]);
    let page = page(repo.path(), PAY_BY_CARD).await;
    assert!(!page.contains("What happens"), "{page}");
    assert!(!page.contains("Business language"), "{page}");
    assert_in_order(main_of(&page), &["<h2>Steps</h2>", "Submits the card"]);
}

// spec: WEB-19
#[tokio::test]
async fn the_use_case_page_has_a_breadcrumb_to_its_domain_and_feature() {
    let repo = generated_repo();
    let page = page(repo.path(), PAY_BY_CARD).await;
    let start = page.find("<nav class=\"crumbs\"").expect(&page);
    let crumbs = &page[start..start + page[start..].find("</nav>").unwrap()];
    assert_in_order(
        crumbs,
        &[
            "href=\"/\">Home</a>",
            "href=\"/domains/billing\"",
            "href=\"/domains/billing/pay-invoice\"",
            "Feature pay-invoice",
            "Use case pay-by-card",
        ],
    );
}

// spec: WEB-20
#[tokio::test]
async fn a_use_case_citing_a_file_changed_since_the_run_is_flagged_stale_with_the_file() {
    let repo = generated_repo();
    std::fs::remove_file(repo.path().join("app/card.rb")).unwrap();
    let page = page(repo.path(), PAY_BY_CARD).await;
    let start = page.find("class=\"badge stale\"").expect(&page);
    let flag = &page[start..start + page[start..].find("</p>").unwrap()];
    assert!(flag.contains("app/card.rb</code> (deleted)"), "{flag}");
    assert_eq!(flag.matches("app/card.rb").count(), 1, "{flag}");
    assert!(!flag.contains("app/invoice.rb"), "{flag}");
}

// spec: WEB-22
#[tokio::test]
async fn a_narrative_judged_unsupported_is_flagged_unbacked_with_the_rationale() {
    let repo = generated_repo();
    let mut by_card = saved_use_case(repo.path(), "pay-by-card");
    by_card.narrative_check = Some(ConfidenceScore::new(
        0.0,
        Some("no card in the code".into()),
    ));
    save_use_cases(repo.path(), &[by_card.clone()]);
    let page = page(repo.path(), PAY_BY_CARD).await;
    assert_in_order(
        main_of(&page),
        &[
            "<h2>What happens</h2>",
            "unbacked",
            "no card in the code",
            "The customer pays",
        ],
    );

    by_card.narrative_check = Some(ConfidenceScore::new(0.5, Some("partly".into())));
    save_use_cases(repo.path(), &[by_card]);
    let page = self::page(repo.path(), PAY_BY_CARD).await;
    assert!(!page.contains("unbacked"), "{page}");
}

// spec: WEB-24
#[tokio::test]
async fn only_a_page_with_a_diagram_loads_the_mermaid_script() {
    let repo = generated_repo();
    let with = page(repo.path(), PAY_BY_CARD).await;
    let script = "<script src=\"/assets/mermaid-12.2.0.min.js\"";
    assert!(with.contains(script), "{with}");
    assert!(
        with.contains("<pre class=\"mermaid\">sequenceDiagram"),
        "{with}"
    );
    for path in [
        "/domains/billing/pay-invoice/retry-job",
        "/domains/billing/pay-invoice",
        "/",
    ] {
        let without = page(repo.path(), path).await;
        assert!(!without.contains("mermaid"), "{path}: {without}");
    }
}

// spec: WEB-24
#[tokio::test]
async fn the_mermaid_script_is_served_compressed_with_a_long_cache() {
    let repo = generated_repo();
    let response = retrodoc_web::router(
        retrodoc_web::Project::default(),
        GeneratedDocs::load(repo.path(), &[]),
        doc_links(),
        Vec::new(),
    )
    .oneshot(
        Request::get("/assets/mermaid-12.2.0.min.js")
            .header("accept-encoding", "gzip")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let header = |name: &str| response.headers()[name].to_str().unwrap().to_string();
    assert_eq!(header("content-encoding"), "gzip");
    assert!(header("content-type").starts_with("text/javascript"));
    assert!(header("cache-control").contains("immutable"));
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..2], [0x1f, 0x8b], "a gzip stream");
}

// spec: WEB-10, WEB-24
#[tokio::test]
async fn the_source_of_a_diagram_is_escaped() {
    let repo = generated_repo();
    let mut by_card = saved_use_case(repo.path(), "pay-by-card");
    by_card.diagram_mermaid = Some("graph TD\n  A</pre><script>alert(1)</script>".into());
    save_use_cases(repo.path(), &[by_card]);
    let page = page(repo.path(), PAY_BY_CARD).await;
    assert!(!page.contains("<script>alert"), "{page}");
    assert!(page.contains("&#60;/pre&#62;&#60;script&#62;"), "{page}");
}

// spec: WEB-25
#[tokio::test]
async fn a_use_case_unknown_or_asked_under_another_feature_is_the_not_found_page() {
    let repo = generated_repo();
    for path in [
        "/domains/billing/pay-invoice/nope",
        "/domains/billing/refund-tool/pay-by-card",
        "/domains/delivery/pay-invoice/pay-by-card",
    ] {
        let (status, page) = get(repo.path(), path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(page.contains("<h1>Not found</h1>"), "{page}");
    }
}

// spec: WEB-18
#[tokio::test]
async fn a_rule_is_shown_with_the_rationale_of_its_confidence() {
    let repo = generated_repo();
    for path in [
        "/domains/billing",
        "/domains/billing/pay-invoice",
        PAY_BY_CARD,
    ] {
        let page = page(repo.path(), path).await;
        assert_in_order(
            main_of(&page),
            &[
                "An invoice is paid within 30 days",
                "30%",
                "backed by code alone",
            ],
        );
    }
}

// spec: WEB-23
#[tokio::test]
async fn the_path_of_a_doc_link_is_encoded() {
    let repo = generated_repo();
    let mut links = DocLinks::default();
    links.links.insert(
        ItemRef::Domain("billing".into()),
        vec![DocLink {
            path: "docs/C# guide?.md".into(),
            heading: Some("Set up".into()),
            contradicts: None,
        }],
    );
    let response = retrodoc_web::router(
        retrodoc_web::Project::default(),
        GeneratedDocs::load(repo.path(), &[]),
        links,
        Vec::new(),
    )
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
    let page = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        page.contains("href=\"/docs/docs/C%23%20guide%3F.md#set-up\""),
        "{page}"
    );
}

async fn mermaid_with(accept_encoding: Option<&str>) -> axum::response::Response {
    let repo = generated_repo();
    let mut request = Request::get("/assets/mermaid-12.2.0.min.js");
    if let Some(encodings) = accept_encoding {
        request = request.header("accept-encoding", encodings);
    }
    retrodoc_web::router(
        retrodoc_web::Project::default(),
        GeneratedDocs::load(repo.path(), &[]),
        doc_links(),
        Vec::new(),
    )
    .oneshot(request.body(Body::empty()).unwrap())
    .await
    .unwrap()
}

// spec: WEB-24
#[tokio::test]
async fn a_client_that_does_not_accept_gzip_is_not_sent_the_compressed_script() {
    for refused in [None, Some("br"), Some("identity"), Some("gzip;q=0, *")] {
        let response = mermaid_with(refused).await;
        assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE, "{refused:?}");
    }
    for accepted in ["*", "br, GZIP;q=0.5"] {
        assert_eq!(
            mermaid_with(Some(accepted)).await.status(),
            StatusCode::OK,
            "{accepted}"
        );
    }
    let response = mermaid_with(Some("gzip, deflate, br")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["vary"], "accept-encoding");
}

/// The English labels of the pages of the items and of reference, none of which a page in French
/// must carry.
const ENGLISH_LABELS: &[&str] = &[
    "Map of this domain",
    ">Features<",
    "Business rules",
    "Existing documentation",
    "states otherwise",
    ">Use cases<",
    "Source files",
    "Entry points",
    "Primary actor",
    "Triggered by",
    "What happens",
    "Technical details",
    ">Steps<",
    ">Diagram<",
    "Source of the diagram",
    "Previous:",
    "Next:",
    "Entities:",
    "Code:",
    "Test:",
    "(human)",
    "(system)",
    ">Home<",
    ">All<",
    ">Filter<",
    "Kind",
    "Feature:",
    "Glossary<",
    "No concept was found",
    "Actors<",
    "People",
    "Systems",
    "Permission rules",
    "Map<",
    "Files<",
    "files cited by the docs",
    "cited by",
    "Cited by",
    "Showing",
    "History",
    "Confidence:",
    "Business language:",
    "not scored",
];

// spec: WEB-77
#[tokio::test]
async fn the_pages_of_the_items_and_of_reference_are_in_french() {
    let repo = generated_repo();
    let mut pages = String::new();
    for path in [
        "/domains/billing",
        "/domains/billing/pay-invoice",
        PAY_BY_CARD,
        "/domains/billing/pay-invoice/retry-job",
        "/rules",
        "/glossary",
        "/actors",
        "/map",
        "/map/billing",
        "/files",
        "/code/app/invoice.rb?lines=4-9",
    ] {
        let (status, page) = get_in(repo.path(), path, Some("fr")).await;
        assert_eq!(status, StatusCode::OK, "{path}: {page}");
        pages.push_str(main_of(&page));
    }
    for label in ENGLISH_LABELS {
        assert!(!pages.contains(label), "{label} left in {pages}");
    }
    for label in [
        "Carte de ce domaine",
        "Ce qui se passe",
        "Règles métier",
        "Acteur principal :",
        "Personnes",
        "Historique",
    ] {
        assert!(pages.contains(label), "{label} missing from {pages}");
    }
}

// spec: WEB-118
#[tokio::test]
async fn the_narrative_of_a_use_case_is_rendered_as_markdown_its_raw_html_as_text() {
    let repo = generated_repo();
    let mut by_card = saved_use_case(repo.path(), "pay-by-card");
    by_card.narrative = Some(
        "The customer pays **by card**.\n\nThen <script>alert(1)</script> [x](javascript:alert(1)) [the guide](docs/guide.md)"
            .into(),
    );
    save_use_cases(repo.path(), &[by_card]);
    let page = page(repo.path(), PAY_BY_CARD).await;
    let start = page.find("<div class=\"narrative\">").expect(&page);
    let narrative = &page[start..start + page[start..].find("</div>").unwrap()];
    assert!(
        narrative.contains("<strong>by card</strong>"),
        "{narrative}"
    );
    assert!(
        narrative.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
        "{narrative}"
    );
    assert!(!narrative.contains("javascript:"), "{narrative}");
    // No collected doc at the path it resolves to (WEB-123).
    assert!(narrative.contains(" the guide</p>"), "{narrative}");
}
