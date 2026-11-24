//! The states every form of the pages shows while it is sent, saved or refused, read from the
//! markup `htmx` acts on: the router asked without a socket over the artifacts a run leaves in
//! `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use retrodoc_core::artifacts::{DomainCluster, DomainMap};
use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
use retrodoc_docs::GeneratedDocs;
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
    retrodoc_core::artifacts::save_features(root, &[feature("pay-invoice")]).unwrap();
    retrodoc_core::artifacts::save_use_cases(root, &[use_case("pay-by-card")]).unwrap();
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
    let docs = GeneratedDocs::load(root, &[]).unwrap().unwrap();
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

/// Asserts that `form` holds an indicator saying `text`, that `htmx` shows while it is sent: the
/// form's, or that of its search box, which sends as it is typed (WEB-39).
fn assert_indicator(form: &str, text: &str) {
    let search_box = form
        .find("<input type=\"search\"")
        .map(|start| opening(&form[start..]));
    assert!(
        search_box.map_or_else(
            || opening(form).contains("hx-indicator=\"find .indicator\""),
            |input| input.contains("hx-indicator=\"next .indicator\"")
        ),
        "{form}"
    );
    assert!(
        form.contains(&format!(
            "<span class=\"indicator\" role=\"status\">{text}</span>"
        )),
        "{form}"
    );
}

// spec: WEB-102, SFM-16
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
        let form = form(&page, "<form class=\"section-form\"");
        assert_indicator(form, "Saving…");
        // Every button of the form, the revert included: `find` would take the first alone.
        assert!(
            opening(form).contains("hx-disabled-elt=\"#review button\""),
            "{form}"
        );
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
        form(&page, "<form class=\"section-form\""),
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

/// What a `POST` sent by `htmx` from the served page got: its status and its body.
async fn post_htmx(
    root: &Path,
    path: &str,
    form: &str,
    lang: Option<&'static str>,
) -> (StatusCode, String) {
    let (status, _, body) = post_htmx_with_headers(root, path, form, lang).await;
    (status, body)
}

/// What a `POST` sent by `htmx` from the served page got: its status, its headers and its body.
async fn post_htmx_with_headers(
    root: &Path,
    path: &str,
    form: &str,
    lang: Option<&'static str>,
) -> (StatusCode, HeaderMap, String) {
    let project = Project {
        name: "Shop".into(),
        lang,
        ..Project::default()
    };
    let docs = GeneratedDocs::load(root, &[]).unwrap().unwrap();
    let request = Request::post(path)
        .header("host", "127.0.0.1:8730")
        .header("origin", "http://127.0.0.1:8730")
        .header("hx-request", "true")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form.to_string()))
        .unwrap();
    let response = retrodoc_web::router(project, Some(docs), DocLinks::default(), Vec::new())
        .oneshot(request)
        .await
        .unwrap();
    let (status, headers) = (response.status(), response.headers().clone());
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, headers, String::from_utf8(body.to_vec()).unwrap())
}

/// Asserts that `form` holds the error `text` above its first button.
fn assert_error_above_buttons(form: &str, text: &str) {
    let error = format!("<p class=\"form-error\" role=\"alert\">{text}</p>");
    let at = form
        .find(&error)
        .unwrap_or_else(|| panic!("no {error} in {form}"));
    assert!(at < form.find("<button").unwrap(), "{form}");
}

// spec: WEB-77, SFM-15
#[tokio::test]
async fn the_errors_of_the_forms_are_worded_in_french() {
    let dir = generated_repo();
    let root = dir.path();
    let (_, body) = post_htmx(root, "/domains/billing", "status=approved", Some("fr")).await;
    assert_error_above_buttons(form(&body, "<form"), "Statut de relecture inconnu.");
    let (_, body) = post_htmx(
        root,
        "/domains/billing",
        "status=&name=N&description=D&narrative=X",
        Some("fr"),
    )
    .await;
    assert_error_above_buttons(form(&body, "<form"), "Seul un cas d&#39;usage a un récit.");
}

// spec: WEB-106
#[tokio::test]
async fn every_page_swaps_an_html_error_and_shows_any_other_in_a_notice() {
    let dir = generated_repo();
    let root = dir.path();
    let page = get(root, "/domains/billing").await;
    assert!(
        page.contains("<script src=\"/assets/forms.js\" defer></script>"),
        "{page}"
    );
    assert!(
        page.contains("<p class=\"page-error\" id=\"page-error\" role=\"alert\" hidden></p>"),
        "{page}"
    );
    // One rule for the page, no hook of its own in a section.
    assert!(!page.contains("before-swap"), "{page}");

    let script = get(root, "/assets/forms.js").await;
    for part in [
        "htmx:beforeSwap",
        "HX-Reswap",
        "htmx:responseError",
        "text/html",
        "page-error",
        "htmx:beforeRequest",
        ".form-error",
    ] {
        assert!(script.contains(part), "{part} in {script}");
    }
}

fn notice(text: &str) -> String {
    format!("<p class=\"notice\" role=\"status\">{text}</p>")
}

// spec: WEB-77, SFM-8
#[tokio::test]
async fn the_saved_notice_is_worded_in_french() {
    let dir = generated_repo();
    let root = dir.path();
    let (_, page) = get_in(root, "/domains/billing?saved=1", Some("fr")).await;
    assert!(page.contains(&notice("Enregistré.")), "{page}");
}

/// What follows the heading of `page`, up to the end of the page.
fn after_heading(page: &str) -> &str {
    let end = page
        .find("</h1>")
        .unwrap_or_else(|| panic!("no heading in {page}"));
    page[end + "</h1>".len()..].trim_start()
}

// spec: SFM-6
#[tokio::test]
async fn a_section_page_shows_its_action_under_its_heading() {
    let dir = generated_repo();
    let root = dir.path();
    for path in [
        "/domains/billing",
        "/domains/billing/pay-invoice",
        "/domains/billing/pay-invoice/pay-by-card",
    ] {
        let page = get(root, path).await;
        let actions = after_heading(&page);
        assert!(
            actions.starts_with("<nav class=\"section-actions\""),
            "{actions}"
        );
        let actions = &actions[..actions.find("</nav>").unwrap()];
        assert_eq!(
            actions,
            "<nav class=\"section-actions\" aria-label=\"Actions on this section\">\
             <a class=\"button\" href=\"#review\">Review</a>"
        );
        assert!(page.contains("id=\"review\""), "{page}");
    }
}

// spec: WEB-77, SFM-6
#[tokio::test]
async fn the_actions_of_a_section_are_worded_in_french() {
    let dir = generated_repo();
    let (_, page) = get_in(dir.path(), "/domains/billing", Some("fr")).await;
    let actions = after_heading(&page);
    assert!(actions.contains(">Relire</a>"), "{actions}");
    assert!(!actions.contains(">Corriger</a>"), "{actions}");
}
