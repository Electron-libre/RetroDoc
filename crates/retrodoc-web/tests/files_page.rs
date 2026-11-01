//! The files page and the link of every page to it, asked of the router
//! without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{
    Actor, ActorKind, BusinessRule, Feature, RuleEvidence, RuleKind, SourceRef, Step, UseCase,
};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::{DomainCluster, DomainMap, SourceHashes};
use retrodoc_render::DocLinks;
use tower::ServiceExt;

fn source(path: &str, start_line: Option<u32>, end_line: Option<u32>) -> SourceRef {
    SourceRef {
        path: path.into(),
        start_line,
        end_line,
    }
}

/// A run whose hashes record `app/invoice.rb`, `app/card.rb`, `lib/util.rb`
/// and `lib/helper.rb`. The feature `pay-invoice` cites `app/invoice.rb`,
/// `app/card.rb`, `lib/helper.rb` and `./app/budget.rb` (no hash recorded);
/// its use case cites `app/invoice.rb` twice, its rule `app/invoice.rb`.
/// `lib/util.rb` is cited by nothing.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut hashes = SourceHashes::default();
    for name in [
        "app/invoice.rb",
        "app/card.rb",
        "lib/util.rb",
        "lib/helper.rb",
    ] {
        hashes.put(name, &hash_content(name));
    }
    hashes.save(root).unwrap();
    retrodoc_pipeline::save_features(
        root,
        &[Feature {
            back_office: false,
            slug: "pay-invoice".into(),
            domain_slug: "billing".into(),
            name: "Pay an invoice".into(),
            description: String::new(),
            source_paths: [
                "app/invoice.rb",
                "app/card.rb",
                "lib/helper.rb",
                "./app/budget.rb",
            ]
            .map(String::from)
            .to_vec(),
            entry_points: Vec::new(),
            confidence: None,
        }],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(
        root,
        &[UseCase {
            slug: "pay-by-card".into(),
            feature_slug: "pay-invoice".into(),
            name: "Pay by card".into(),
            description: String::new(),
            steps: vec![Step {
                order: 1,
                description: "Charges the card".into(),
                actor: Actor {
                    name: "Gateway".into(),
                    kind: ActorKind::System,
                },
                action: String::new(),
                source_refs: vec![
                    source("app/invoice.rb", Some(1), Some(2)),
                    source("app/invoice.rb", Some(8), None),
                ],
            }],
            entry_points: Vec::new(),
            primary_actor: None,
            narrative: None,
            business_language: None,
            diagram_mermaid: None,
            confidence: None,
            narrative_check: None,
        }],
    )
    .unwrap();
    retrodoc_pipeline::save_rules(
        root,
        &[BusinessRule {
            slug: "pay-invoice-1".into(),
            statement: "A late invoice is charged".into(),
            kind: RuleKind::Limit,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: vec![RuleEvidence::Code {
                source: source("app/invoice.rb", Some(1), Some(2)),
            }],
            confidence: None,
        }],
    )
    .unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            description: String::new(),
            ..DomainCluster::default()
        }],
    }
    .save(root)
    .unwrap();
    dir
}

async fn get(docs: Option<GeneratedDocs>, path: &str) -> (StatusCode, String) {
    let response = retrodoc_web::router(docs, DocLinks::default())
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn files_page(root: &Path) -> String {
    let (status, body) = get(GeneratedDocs::load(root, &[]), "/files").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// The row of the tree whose `id` is `path`.
fn row<'a>(page: &'a str, path: &str) -> &'a str {
    let start = page
        .find(&format!("<tr id=\"{path}\""))
        .unwrap_or_else(|| panic!("no row {path}: {page}"));
    let end = start + page[start..].find("</tr>").unwrap();
    &page[start..end]
}

// spec: WEB-34
#[tokio::test]
async fn the_files_page_shows_the_tree_with_the_share_of_cited_files() {
    let repo = generated_repo();
    let page = files_page(repo.path()).await;
    assert!(row(&page, "app/").contains("3/3 cited"), "{page}");
    assert!(row(&page, "lib/").contains("1/2 cited"), "{page}");
    assert!(page.contains("4/5 files cited"), "{page}");
    assert!(
        row(&page, "app/invoice.rb").contains("cited by 3"),
        "{page}"
    );
    assert!(row(&page, "app/card.rb").contains("cited by 1"), "{page}");
    let order: Vec<usize> = [
        "app/",
        "app/budget.rb",
        "app/card.rb",
        "app/invoice.rb",
        "lib/",
    ]
    .iter()
    .map(|path| page.find(&format!("<tr id=\"{path}\"")).unwrap())
    .collect();
    assert!(order.is_sorted(), "{page}");
}

// spec: WEB-34
#[tokio::test]
async fn a_cited_file_whose_hash_was_not_recorded_is_in_the_tree() {
    let repo = generated_repo();
    let page = files_page(repo.path()).await;
    assert!(row(&page, "app/budget.rb").contains("cited by 1"), "{page}");
    assert!(!page.contains("./app"), "{page}");
}

// spec: WEB-35
#[tokio::test]
async fn a_cited_file_links_to_its_page() {
    let repo = generated_repo();
    let page = files_page(repo.path()).await;
    assert!(
        row(&page, "app/invoice.rb").contains("<a href=\"/code/app/invoice.rb\">invoice.rb</a>"),
        "{page}"
    );
}

// spec: WEB-36
#[tokio::test]
async fn a_file_no_doc_cites_is_flagged_with_no_link() {
    let repo = generated_repo();
    let page = files_page(repo.path()).await;
    let util = row(&page, "lib/util.rb");
    assert!(util.contains("not cited"), "{util}");
    assert!(!util.contains("<a "), "{util}");
}

// spec: WEB-37
#[tokio::test]
async fn every_page_links_to_the_files_page() {
    let repo = generated_repo();
    for path in [
        "/",
        "/files",
        "/domains/billing",
        "/domains/billing/pay-invoice",
        "/domains/billing/pay-invoice/pay-by-card",
        "/nope",
    ] {
        let (_, page) = get(GeneratedDocs::load(repo.path(), &[]), path).await;
        assert!(page.contains("<a href=\"/files\">"), "{path}: {page}");
    }
    let (status, page) = get(None, "/files").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("retrodoc generate"), "{page}");
    assert!(page.contains("<a href=\"/files\">"), "{page}");
}
