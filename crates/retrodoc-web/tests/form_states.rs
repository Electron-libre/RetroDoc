//! The states every form of the pages shows while it is sent, saved or refused, read from the
//! markup `htmx` acts on: the router asked without a socket over the artifacts a run leaves in
//! `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
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

/// A run over `billing`: the feature `pay-invoice` and its use case `pay-by-card`.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(root, &[feature("pay-invoice")]).unwrap();
    retrodoc_pipeline::save_use_cases(root, &[use_case("pay-by-card")]).unwrap();
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

async fn get_in(root: &Path, path: &str, lang: Option<&'static str>) -> (StatusCode, String) {
    let project = Project {
        name: "Shop".into(),
        lang,
        ..Project::default()
    };
    let docs = GeneratedDocs::load(root, &[]).unwrap();
    let response = retrodoc_web::router(project, Some(docs), DocLinks::default(), Vec::new())
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn get(root: &Path, path: &str) -> String {
    let (status, page) = get_in(root, path, Some("en")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

/// The first form of `page` whose opening tag starts with `opening`, up to its end.
fn form<'a>(page: &'a str, opening: &str) -> &'a str {
    let start = page
        .find(opening)
        .unwrap_or_else(|| panic!("no {opening} in {page}"));
    let end = start + page[start..].find("</form>").unwrap();
    &page[start..end]
}

/// The opening tag of `form`.
fn opening(form: &str) -> &str {
    &form[..form.find('>').unwrap()]
}

/// Asserts that `form` holds an indicator saying `text`, that `htmx` shows while it is sent.
fn assert_indicator(form: &str, text: &str) {
    assert!(
        opening(form).contains("hx-indicator=\"find .indicator\""),
        "{form}"
    );
    assert!(
        form.contains(&format!(
            "<span class=\"indicator\" role=\"status\">{text}</span>"
        )),
        "{form}"
    );
}

// spec: WEB-102
#[tokio::test]
async fn the_forms_that_write_show_that_they_are_sent_and_disable_their_buttons() {
    let dir = generated_repo();
    let root = dir.path();
    for path in [
        "/domains/billing",
        "/domains/billing/pay-invoice",
        "/domains/billing/pay-invoice/pay-by-card",
    ] {
        let page = get(root, path).await;
        for start in [
            "<form class=\"filters\" method=\"post\"",
            "<form class=\"edit-fields\"",
        ] {
            let form = form(&page, start);
            assert_indicator(form, "Saving…");
            assert!(
                opening(form).contains("hx-disabled-elt=\"find button\""),
                "{form}"
            );
        }
    }
}

// spec: WEB-102
#[tokio::test]
async fn the_search_and_the_filters_show_that_they_load_with_their_controls_enabled() {
    let dir = generated_repo();
    let root = dir.path();
    for (path, start) in [
        ("/", "<form class=\"search\""),
        ("/debt", "<form class=\"filters\" action=\"/debt\""),
        ("/rules", "<form class=\"filters\" action=\"/rules\""),
    ] {
        let page = get(root, path).await;
        let form = form(&page, start);
        assert_indicator(form, "Loading…");
        assert!(!opening(form).contains("hx-disabled-elt"), "{form}");
    }
}

// spec: WEB-77, WEB-102
#[tokio::test]
async fn the_indicators_are_worded_in_french() {
    let dir = generated_repo();
    let root = dir.path();
    let (_, page) = get_in(root, "/domains/billing", Some("fr")).await;
    assert_indicator(
        form(&page, "<form class=\"edit-fields\""),
        "Enregistrement…",
    );
    assert_indicator(form(&page, "<form class=\"search\""), "Chargement…");
}

// spec: WEB-102
#[tokio::test]
async fn htmx_injects_no_indicator_style_of_its_own() {
    let dir = generated_repo();
    let page = get(dir.path(), "/").await;
    assert!(
        page.contains(r#"<meta name="htmx-config" content='{"includeIndicatorStyles":false}'>"#),
        "{page}"
    );
}
