//! The glossary page and the link of every page to it, asked of the router
//! without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{
    Actor, ActorKind, BusinessRule, Feature, RuleKind, SourceRef, Step, UseCase,
};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::glossary::ModelFile;
use retrodoc_pipeline::{DomainCluster, DomainMap, Entity, Glossary};
use retrodoc_render::{anchor, DocLinks};
use tower::ServiceExt;

fn use_case(slug: &str, cited: &[&str]) -> UseCase {
    UseCase {
        slug: slug.into(),
        feature_slug: "pay-invoice".into(),
        name: format!("Use case {slug}"),
        description: String::new(),
        steps: vec![Step {
            order: 1,
            description: "Does it".into(),
            actor: Actor {
                name: "Clerk".into(),
                kind: ActorKind::Human,
            },
            action: String::new(),
            source_refs: cited
                .iter()
                .map(|path| SourceRef {
                    path: (*path).into(),
                    start_line: Some(1),
                    end_line: None,
                })
                .collect(),
        }],
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        diagram_mermaid: None,
        confidence: None,
        narrative_check: None,
    }
}

fn model(name: &str, description: &str) -> ModelFile {
    ModelFile {
        content_hash: String::new(),
        entities: vec![Entity {
            name: name.into(),
            description: description.into(),
            attributes: Vec::new(),
            associations: Vec::new(),
        }],
    }
}

/// A run with the concepts `invoice`, `Card`, `Customer` and `Zone`: the use
/// case `pay-by-card` cites the files of `invoice` and `Card`, the rule of
/// `pay-late` names `Customer`, nothing uses `Zone`.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(
        root,
        &[Feature {
            back_office: false,
            slug: "pay-invoice".into(),
            domain_slug: "billing".into(),
            name: "Pay an invoice".into(),
            description: String::new(),
            source_paths: vec!["app/invoice.rb".into()],
            entry_points: Vec::new(),
            confidence: None,
        }],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(
        root,
        &[
            use_case("pay-by-card", &["app/invoice.rb", "app/card.rb"]),
            use_case("pay-late", &[]),
        ],
    )
    .unwrap();
    retrodoc_pipeline::save_rules(
        root,
        &[BusinessRule {
            slug: "pay-invoice-1".into(),
            statement: "A late customer pays a fee".into(),
            kind: RuleKind::Limit,
            entities: vec!["customer".into()],
            use_cases: vec!["pay-late".into()],
            evidence: Vec::new(),
            confidence: None,
        }],
    )
    .unwrap();
    Glossary {
        models: [
            ("app/invoice.rb", model("invoice", "What a customer owes")),
            ("app/card.rb", model("Card", "A way to pay")),
            ("app/customer.rb", model("Customer", "Who buys")),
            ("app/zone.rb", model("Zone", "Where orders go")),
        ]
        .into_iter()
        .map(|(path, file)| (PathBuf::from(path), file))
        .collect(),
        tests: Vec::new(),
    }
    .save(root)
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

async fn get(root: &Path, path: &str) -> (StatusCode, String) {
    let response = retrodoc_web::router(
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

async fn glossary(root: &Path) -> String {
    let (status, page) = get(root, "/glossary").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

/// The section of the concept `name`, from its heading to the next one.
fn concept<'a>(page: &'a str, name: &str) -> &'a str {
    let start = page
        .find(&format!("id=\"{}\"", anchor(name)))
        .unwrap_or_else(|| panic!("no concept {name} in {page}"));
    let rest = &page[start..];
    &rest[..rest[1..].find("<h2").map_or(rest.len(), |end| end + 1)]
}

// spec: WEB-44
#[tokio::test]
async fn every_page_links_to_the_glossary() {
    let repo = generated_repo();
    for path in ["/", "/files", "/glossary", "/unknown"] {
        let (_, page) = get(repo.path(), path).await;
        let header = &page[page.find("<header").unwrap()..page.find("</header>").unwrap()];
        assert!(header.contains("href=\"/glossary\""), "{path}: {header}");
    }
}

// spec: WEB-45
#[tokio::test]
async fn the_glossary_lists_every_concept_by_name_with_its_definition() {
    let repo = generated_repo();
    let page = glossary(repo.path()).await;
    let positions: Vec<usize> = ["Card", "Customer", "invoice", "Zone"]
        .iter()
        .map(|name| page.find(&format!(">{name}</h2>")).expect(name))
        .collect();
    assert!(positions.is_sorted(), "{positions:?} in {page}");
    assert!(concept(&page, "invoice").contains("What a customer owes"));
    assert!(concept(&page, "Zone").contains("Where orders go"));
}

// spec: WEB-45
#[tokio::test]
async fn a_repository_with_no_glossary_says_no_concept_was_found() {
    let repo = generated_repo();
    std::fs::remove_file(retrodoc_pipeline::Artifact::Glossary.path(repo.path())).unwrap();
    let page = glossary(repo.path()).await;
    assert!(page.contains("No concept"), "{page}");
}

// spec: WEB-46
#[tokio::test]
async fn each_concept_lists_the_use_cases_that_use_it() {
    let repo = generated_repo();
    let page = glossary(repo.path()).await;
    let by_card = "href=\"/domains/billing/pay-invoice/pay-by-card\"";
    let late = "href=\"/domains/billing/pay-invoice/pay-late\"";
    // A step cites its file.
    assert!(concept(&page, "invoice").contains(by_card), "{page}");
    assert!(concept(&page, "Card").contains(by_card), "{page}");
    assert!(!concept(&page, "Card").contains(late), "{page}");
    // A rule of the use case names it.
    assert!(concept(&page, "Customer").contains(late), "{page}");
    assert!(!concept(&page, "Customer").contains(by_card), "{page}");
    assert!(
        !concept(&page, "Zone").contains("href=\"/domains/"),
        "{page}"
    );
}
