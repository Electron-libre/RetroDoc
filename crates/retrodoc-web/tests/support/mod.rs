//! The run of the debt tests: two domains, their features and use cases saved as a run leaves
//! them, the reviews and edits saved as the forms save them, and the pages asked of the router
//! without a socket.
#![allow(dead_code, unreachable_pub)]

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::artifacts::{DomainCluster, DomainMap};
use retrodoc_core::model::{Actor, ActorKind, ConfidenceScore, Feature, SourceRef, Step, UseCase};
use retrodoc_docs::edits::{self, Fields};
use retrodoc_docs::reviews::{self, Section, SectionKind, Status};
use retrodoc_docs::GeneratedDocs;
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use tower::ServiceExt;

pub fn feature(slug: &str, domain: &str, name: &str) -> Feature {
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

pub fn use_case(slug: &str, feature: &str, name: &str, confidence: Option<f32>) -> UseCase {
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

pub fn use_cases() -> Vec<UseCase> {
    vec![
        use_case("refund", "pay-invoice", "Refund", Some(0.3)),
        use_case("pay-by-card", "pay-invoice", "Pay by card", None),
        use_case("ship", "ship-order", "Ship", Some(0.9)),
    ]
}

/// `billing` (feature `pay-invoice`: `Refund` low at 30% and set apart for it, `Pay by card` not
/// scored) and `shipping` (feature `ship-order`, use case `Ship` at 90%); no git history.
pub fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_core::artifacts::save_features(
        root,
        &[
            feature("pay-invoice", "billing", "Pay an invoice"),
            feature("ship-order", "shipping", "Ship an order"),
        ],
    )
    .unwrap();
    retrodoc_core::artifacts::save_use_cases(root, &use_cases()).unwrap();
    save_domains(root, "");
    dir
}

pub fn save_domains(root: &Path, description: &str) {
    DomainMap {
        domains: ["billing", "shipping"]
            .map(|slug| DomainCluster {
                slug: slug.into(),
                name: format!("{}{}", slug[..1].to_uppercase(), &slug[1..]),
                description: description.into(),
                ..DomainCluster::default()
            })
            .to_vec(),
    }
    .save(root)
    .unwrap();
}

/// A later run that rewrites the domains, the features and the use cases: every review is
/// outdated, and a section marked wrong is listed again (FIX-5).
pub fn rewrite_run(root: &Path) {
    save_domains(root, "Rewritten.");
    let features: Vec<Feature> = [
        ("pay-invoice", "billing", "Pay an invoice"),
        ("ship-order", "shipping", "Ship an order"),
    ]
    .map(|(slug, domain, name)| Feature {
        description: "Rewritten.".into(),
        ..feature(slug, domain, name)
    })
    .to_vec();
    retrodoc_core::artifacts::save_features(root, &features).unwrap();
    let rewritten: Vec<UseCase> = use_cases()
        .into_iter()
        .map(|u| UseCase {
            description: "Rewritten.".into(),
            ..u
        })
        .collect();
    retrodoc_core::artifacts::save_use_cases(root, &rewritten).unwrap();
}

pub fn review(root: &Path, kind: SectionKind, id: &str, status: Status, comment: &str) {
    let docs = GeneratedDocs::load(root, &[]).unwrap().unwrap();
    reviews::record(root, &docs, &Section::new(kind, id), status, comment).unwrap();
}

pub async fn get(root: &Path, path: &str) -> String {
    let response = retrodoc_web::router(
        Project::default(),
        GeneratedDocs::load(root, &[]).unwrap(),
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
pub fn items(page: &str) -> Vec<String> {
    let start = page.find("id=\"debt-list\"").expect(page);
    page[start..]
        .split("<li class=\"debt-item\"")
        .skip(1)
        .map(str::to_string)
        .collect()
}

/// The kind and the link of each item, in order.
pub fn listed(page: &str) -> Vec<(String, String)> {
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

pub fn pair(kind: &str, href: &str) -> (String, String) {
    (kind.to_string(), href.to_string())
}

pub fn edit_name(root: &Path, kind: SectionKind, id: &str, name: &str) {
    let docs = GeneratedDocs::load(root, &[]).unwrap().unwrap();
    let fields = Fields {
        name: Some(name.into()),
        ..Fields::default()
    };
    edits::record(root, &docs, &Section::new(kind, id), fields).unwrap();
}
