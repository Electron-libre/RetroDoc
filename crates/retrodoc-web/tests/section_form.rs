//! The one form of a section page (`specs/section-form.md`): its review and its human edit, sent
//! together, asked of the router without a socket over the artifacts a run leaves in
//! `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::artifacts::{DomainCluster, DomainMap};
use retrodoc_core::model::{Feature, UseCase};
use retrodoc_docs::edits::{self, Fields};
use retrodoc_docs::reviews::{self, Section, SectionKind, Status};
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
        confidence: None,
    }
}

fn use_case() -> UseCase {
    UseCase {
        slug: "pay-by-card".into(),
        feature_slug: "pay-invoice".into(),
        name: "Pay by card".into(),
        description: "The customer pays".into(),
        steps: Vec::new(),
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: Some("The clerk takes the card.".into()),
        business_language: None,
        diagram_mermaid: None,
        confidence: None,
        narrative_check: None,
    }
}

/// A git repository of `user.name` `Ada` holding a run over `billing`: the features
/// `pay-invoice` (the use case `pay-by-card`) and `refund`.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for args in [&["init", "-q"][..], &["config", "user.name", "Ada"]] {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .unwrap();
        assert!(status.success());
    }
    retrodoc_core::artifacts::save_features(root, &[feature("pay-invoice"), feature("refund")])
        .unwrap();
    retrodoc_core::artifacts::save_use_cases(root, &[use_case()]).unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            description: "Invoices".into(),
            ..DomainCluster::default()
        }],
    }
    .save(root)
    .unwrap();
    dir
}

const DOMAIN: &str = "/domains/billing";
const FEATURE: &str = "/domains/billing/pay-invoice";
const USE_CASE: &str = "/domains/billing/pay-invoice/pay-by-card";
const PAGES: [&str; 3] = [DOMAIN, FEATURE, USE_CASE];

fn load(root: &Path) -> GeneratedDocs {
    GeneratedDocs::load(root, &[]).unwrap().unwrap()
}

fn pay_invoice() -> Section {
    Section::new(SectionKind::Feature, "pay-invoice")
}

fn pay_by_card() -> Section {
    Section::use_case("pay-invoice", "pay-by-card")
}

fn edit_name(root: &Path, section: &Section, name: &str) {
    let fields = Fields {
        name: Some(name.into()),
        ..Fields::default()
    };
    edits::record(root, &load(root), section, fields).unwrap();
}

fn app(root: &Path, lang: &'static str) -> axum::Router {
    let project = Project {
        lang: Some(lang),
        ..Project::default()
    };
    retrodoc_web::router(
        project,
        GeneratedDocs::load(root, &[]).unwrap(),
        DocLinks::default(),
        Vec::new(),
    )
}

async fn get(root: &Path, path: &str) -> String {
    let response = app(root, "en")
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{path}");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

/// What the router answered a `POST`.
struct Answer {
    status: StatusCode,
    location: Option<String>,
    redirect: Option<String>,
    reswap: Option<String>,
    body: String,
}

/// The browser of the served page posting `form` to `path`, sent by `htmx` when `htmx`.
async fn post(root: &Path, path: &str, form: &str, htmx: bool) -> Answer {
    let mut request = Request::post(path)
        .header("host", "127.0.0.1:8730")
        .header("origin", "http://127.0.0.1:8730")
        .header("content-type", "application/x-www-form-urlencoded");
    if htmx {
        request = request.header("hx-request", "true");
    }
    let response = app(root, "en")
        .oneshot(request.body(Body::from(form.to_string())).unwrap())
        .await
        .unwrap();
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .map(|v| v.to_str().unwrap().to_string())
    };
    let (status, location, redirect, reswap) = (
        response.status(),
        header("location"),
        header("hx-redirect"),
        header("hx-reswap"),
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    Answer {
        status,
        location,
        redirect,
        reswap,
        body: String::from_utf8(body.to_vec()).unwrap(),
    }
}

/// The form block of a section page.
fn form_block(page: &str) -> &str {
    let start = page
        .find("<section class=\"review\"")
        .unwrap_or_else(|| panic!("no form block in {page}"));
    let end = start + page[start..].find("</section>").unwrap();
    &page[start..end]
}

/// The reviews saved: section, status, comment.
fn reviews_saved(root: &Path) -> Vec<(Section, Status, String)> {
    reviews::load(root)
        .unwrap()
        .reviews()
        .iter()
        .map(|r| (r.section.clone(), r.status, r.comment.clone()))
        .collect()
}

/// The edits saved: section and fields.
fn edits_saved(root: &Path) -> Vec<(Section, Fields)> {
    edits::load(root)
        .unwrap()
        .edits()
        .iter()
        .map(|e| (e.section.clone(), e.fields.clone()))
        .collect()
}

/// The bytes of both files of human work, `None` for an absent one.
fn both_files(root: &Path) -> [Option<Vec<u8>>; 2] {
    [edits::edits_path(root), reviews::reviews_path(root)].map(|path| std::fs::read(path).ok())
}

// spec: SFM-1
#[tokio::test]
async fn every_section_page_holds_one_form_of_its_review_and_its_text() {
    let dir = generated_repo();
    for path in PAGES {
        let page = get(dir.path(), path).await;
        let block = form_block(&page);
        assert!(block.contains("id=\"review\""), "{block}");
        assert!(
            block.contains(&format!(
                "method=\"post\" action=\"{path}\" hx-post=\"{path}\""
            )),
            "{block}"
        );
        assert_eq!(page.matches("<form class=\"section-form\"").count(), 1);
        assert!(!page.contains("action=\"/edit/"), "{page}");
        for value in ["", "validated", "wrong", "to_review"] {
            let radio = format!("<input type=\"radio\" name=\"status\" value=\"{value}\"");
            assert!(block.contains(&radio), "{radio} in {block}");
        }
        for field in [
            "<textarea name=\"comment\"",
            "<input name=\"name\"",
            "<textarea name=\"description\"",
        ] {
            assert!(block.contains(field), "{field} in {block}");
        }
        assert_eq!(
            block.contains("name=\"narrative\""),
            path == USE_CASE,
            "{block}"
        );
        assert_eq!(block.matches("<button type=\"submit\"").count(), 1);
    }
}

// spec: SFM-1
#[tokio::test]
async fn the_form_is_filled_with_the_review_and_the_text_the_page_shows() {
    let dir = generated_repo();
    let root = dir.path();
    edit_name(root, &pay_by_card(), "Pay with a card");
    reviews::record(
        root,
        &load(root),
        &pay_by_card(),
        Status::Wrong,
        "Misses cash",
    )
    .unwrap();

    let page = get(root, USE_CASE).await;

    let block = form_block(&page);
    assert!(
        block.contains("name=\"status\" value=\"wrong\" checked>"),
        "{block}"
    );
    assert!(block.contains(">Misses cash</textarea>"), "{block}");
    assert!(
        block.contains("<input name=\"name\" value=\"Pay with a card\">"),
        "{block}"
    );
    assert!(block.contains(">The customer pays</textarea>"), "{block}");
    assert!(
        block.contains(">The clerk takes the card.</textarea>"),
        "{block}"
    );
}

// spec: SFM-H1, SFM-3, SFM-9
#[tokio::test]
async fn validating_a_section_records_its_review_and_writes_no_edit() {
    let dir = generated_repo();
    let root = dir.path();

    let answer = post(
        root,
        FEATURE,
        "status=validated&comment=&name=Feature+pay-invoice&description=What+pay-invoice+does",
        true,
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        answer.redirect.as_deref(),
        Some("/domains/billing/pay-invoice?saved=1")
    );
    let answer = post(root, FEATURE, "status=validated", false).await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    assert_eq!(
        answer.location.as_deref(),
        Some("/domains/billing/pay-invoice?saved=1")
    );
    assert_eq!(
        reviews_saved(root),
        [(pay_invoice(), Status::Validated, String::new())]
    );
    assert!(!edits::edits_path(root).exists());
}

// spec: SFM-3
#[tokio::test]
async fn a_review_alone_leaves_the_edit_as_it_was() {
    let dir = generated_repo();
    let root = dir.path();
    edit_name(root, &pay_invoice(), "Pay an invoice");
    let before = both_files(root)[0].clone();

    post(
        root,
        FEATURE,
        "status=to_review&comment=Check&name=Pay+an+invoice&description=What+pay-invoice+does",
        false,
    )
    .await;
    // A form that sends no field of the text leaves the edit too.
    post(root, FEATURE, "status=to_review&comment=Check", false).await;

    assert_eq!(both_files(root)[0], before);
    assert_eq!(
        reviews_saved(root),
        [(pay_invoice(), Status::ToReview, "Check".into())]
    );
}

// spec: SFM-3
#[tokio::test]
async fn no_status_removes_the_review_and_keeps_the_edit() {
    let dir = generated_repo();
    let root = dir.path();
    edit_name(root, &pay_invoice(), "Pay an invoice");
    reviews::record(root, &load(root), &pay_invoice(), Status::Wrong, "").unwrap();

    post(
        root,
        FEATURE,
        "status=&comment=&name=Pay+an+invoice&description=What+pay-invoice+does",
        false,
    )
    .await;

    assert_eq!(reviews_saved(root), []);
    assert_eq!(edits_saved(root).len(), 1);
}

// spec: SFM-12
#[tokio::test]
async fn a_save_to_a_section_the_run_lacks_is_not_found_and_writes_nothing() {
    let dir = generated_repo();
    let root = dir.path();
    for path in [
        "/domains/shipping",
        "/domains/billing/unknown",
        "/domains/shipping/pay-invoice",
        "/domains/billing/refund/pay-by-card",
    ] {
        let answer = post(root, path, "status=wrong&name=X", false).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{path}");
    }
    assert_eq!(both_files(root), [None, None]);
}

// spec: SFM-13
#[tokio::test]
async fn a_bad_status_or_a_narrative_outside_a_use_case_writes_neither_file() {
    let dir = generated_repo();
    let root = dir.path();
    for (path, form) in [
        (FEATURE, "status=approved&name=Other"),
        (DOMAIN, "status=validated&narrative=The+clerk+pays."),
        (FEATURE, "status=validated&narrative=The+clerk+pays."),
    ] {
        let answer = post(root, path, form, false).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{form}");
    }
    assert_eq!(both_files(root), [None, None]);
}

// spec: SFM-14
#[tokio::test]
async fn a_file_that_cannot_be_read_fails_the_save_and_leaves_both_files() {
    for unreadable in [edits::edits_path, reviews::reviews_path] {
        let dir = generated_repo();
        let root = dir.path();
        edit_name(root, &pay_invoice(), "Pay an invoice");
        reviews::record(root, &load(root), &pay_invoice(), Status::ToReview, "").unwrap();
        std::fs::write(unreadable(root), "{ not json").unwrap();
        let before = both_files(root);

        for htmx in [false, true] {
            let answer = post(root, FEATURE, "status=validated&name=Paid", htmx).await;
            assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
            assert!(answer.body.contains("cannot be read"), "{}", answer.body);
        }
        assert_eq!(both_files(root), before);
    }
}

// spec: SFM-15
#[tokio::test]
async fn a_refused_htmx_save_answers_the_form_as_sent_with_its_error() {
    let dir = generated_repo();
    let root = dir.path();
    reviews::record(
        root,
        &load(root),
        &pay_invoice(),
        Status::Wrong,
        "Misses refunds",
    )
    .unwrap();

    let answer = post(
        root,
        FEATURE,
        "status=approved&comment=Other&name=Paid+invoice&description=Kept",
        true,
    )
    .await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.reswap.as_deref(), Some("outerHTML"));
    let body = &answer.body;
    assert!(body.starts_with("<section class=\"review\""), "{body}");
    let (above, form) = body.split_at(body.find("<form").unwrap());
    assert!(above.contains(">Wrong</span>"), "{body}");
    assert!(above.contains("Misses refunds"), "{body}");
    assert!(form.contains(">Other</textarea>"), "{body}");
    assert!(form.contains("value=\"Paid invoice\""), "{body}");
    assert!(form.contains(">Kept</textarea>"), "{body}");
    assert!(
        form.contains("<details class=\"text-fields\" open>"),
        "{body}"
    );
    let error = form.find("role=\"alert\">Unknown review status.").unwrap();
    assert!(
        error < form.find("<button type=\"submit\"").unwrap(),
        "{body}"
    );

    let missing = post(
        root,
        "/domains/billing/unknown",
        "status=wrong&name=X",
        true,
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert!(
        missing.body.contains("value=\"wrong\" checked>"),
        "{}",
        missing.body
    );
}

// spec: SFM-H2, SFM-3, SFM-4
#[tokio::test]
async fn validating_and_fixing_in_one_save_keeps_the_review_up_to_date() {
    let dir = generated_repo();
    let root = dir.path();
    let form = "status=validated&comment=the+customer+pays%2C+not+the+clerk&name=Pay+by+card\
        &description=The+customer+pays+by+card&narrative=The+clerk+takes+the+card.";

    let answer = post(root, USE_CASE, form, false).await;

    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    assert_eq!(
        answer.location.as_deref(),
        Some("/domains/billing/pay-invoice/pay-by-card?saved=1")
    );
    let description = Fields {
        description: Some("The customer pays by card".into()),
        ..Fields::default()
    };
    assert_eq!(edits_saved(root), [(pay_by_card(), description)]);
    assert_eq!(
        reviews_saved(root),
        [(
            pay_by_card(),
            Status::Validated,
            "the customer pays, not the clerk".into()
        )]
    );
    let page = get(root, USE_CASE).await;
    assert!(!page.contains("Outdated review"), "{page}");
}

// spec: SFM-3, SFM-4
#[tokio::test]
async fn correcting_a_validated_section_writes_the_review_again() {
    let dir = generated_repo();
    let root = dir.path();
    reviews::record(root, &load(root), &pay_invoice(), Status::Validated, "").unwrap();

    post(
        root,
        FEATURE,
        "status=validated&name=Pay+an+invoice&description=What+pay-invoice+does",
        false,
    )
    .await;

    assert_eq!(edits_saved(root).len(), 1);
    let page = get(root, FEATURE).await;
    assert!(page.contains("Pay an invoice"), "{page}");
    assert!(!page.contains("Outdated review"), "{page}");
}

// spec: SFM-10, SFM-11
#[tokio::test]
async fn the_edit_keeps_only_the_fields_that_change_the_generated_text_cleaned() {
    let dir = generated_repo();
    let root = dir.path();

    post(
        root,
        USE_CASE,
        "status=&name=+Pay+by+card+&description=&narrative=Line+one%0D%0ALine+two%0D%0A",
        false,
    )
    .await;

    let narrative = Fields {
        narrative: Some("Line one\nLine two".into()),
        ..Fields::default()
    };
    assert_eq!(edits_saved(root), [(pay_by_card(), narrative)]);
}

// spec: SFM-11
#[tokio::test]
async fn emptying_the_only_edited_field_removes_the_edit() {
    let dir = generated_repo();
    let root = dir.path();
    edit_name(root, &pay_invoice(), "Pay an invoice");

    post(
        root,
        FEATURE,
        "status=&name=&description=What+pay-invoice+does",
        false,
    )
    .await;

    assert_eq!(edits_saved(root), []);
}

// spec: SFM-5
#[tokio::test]
async fn the_back_button_removes_the_edit_and_saves_the_review_as_sent() {
    let dir = generated_repo();
    let root = dir.path();
    edit_name(root, &pay_invoice(), "Pay an invoice");
    let page = get(root, FEATURE).await;
    assert!(
        form_block(&page).contains("name=\"action\" value=\"revert\""),
        "{page}"
    );

    post(
        root,
        FEATURE,
        "status=wrong&comment=&name=Pay+an+invoice&description=What+pay-invoice+does&action=revert",
        false,
    )
    .await;

    assert_eq!(edits_saved(root), []);
    assert_eq!(
        reviews_saved(root),
        [(pay_invoice(), Status::Wrong, String::new())]
    );
    let page = get(root, FEATURE).await;
    assert!(!page.contains("Outdated review"), "{page}");
}

// spec: SFM-8
#[tokio::test]
async fn a_page_asked_after_a_save_says_it_is_saved() {
    let dir = generated_repo();
    let root = dir.path();
    let notice = "<p class=\"notice\" role=\"status\">Saved.</p>";
    for path in PAGES {
        let page = get(root, &format!("{path}?saved=1")).await;
        assert!(form_block(&page).contains(notice), "{page}");
        for other in ["review", "edit", "0"] {
            let page = get(root, &format!("{path}?saved={other}")).await;
            assert!(!page.contains("class=\"notice\""), "{page}");
        }
    }
    std::fs::write(reviews::reviews_path(root), "{ not json").unwrap();
    let page = get(root, &format!("{DOMAIN}?saved=1")).await;
    assert!(!page.contains("class=\"notice\""), "{page}");
}

// spec: SFM-2
#[tokio::test]
async fn the_fields_of_the_text_unfold_for_a_section_marked_wrong() {
    let dir = generated_repo();
    let root = dir.path();
    let folded = "<details class=\"text-fields\">";
    let page = get(root, FEATURE).await;
    assert!(form_block(&page).contains(folded), "{page}");

    reviews::record(root, &load(root), &pay_invoice(), Status::Wrong, "").unwrap();

    let page = get(root, FEATURE).await;
    assert!(
        form_block(&page).contains("<details class=\"text-fields\" open>"),
        "{page}"
    );
}

// spec: SFM-2 (internal: the script runs in a browser; the test reads what the page loads)
#[tokio::test]
async fn choosing_wrong_or_validated_unfolds_the_fields_of_the_text() {
    let dir = generated_repo();
    let script = get(dir.path(), "/assets/forms.js").await;
    for part in [
        "name !== \"status\"",
        "UNFOLDING_STATUSES = [\"wrong\", \"validated\"]",
        "UNFOLDING_STATUSES.includes(input.value)",
        "details.text-fields",
        ".open = true",
    ] {
        assert!(script.contains(part), "{part} in {script}");
    }
}

// spec: SFM-3
#[tokio::test]
async fn a_review_from_a_page_shown_before_an_edit_keeps_that_edit() {
    let dir = generated_repo();
    let root = dir.path();
    let page = get(root, FEATURE).await;
    let block = form_block(&page);
    assert!(
        block.contains("<input type=\"hidden\" name=\"shown_name\" value=\"Feature pay-invoice\">"),
        "{block}"
    );
    // Another tab saves a name while this page still shows the generated one.
    edit_name(root, &pay_invoice(), "Pay an invoice");

    post(
        root,
        FEATURE,
        "status=validated&name=Feature+pay-invoice&description=What+pay-invoice+does\
         &shown_name=Feature+pay-invoice&shown_description=What+pay-invoice+does",
        false,
    )
    .await;

    assert_eq!(edits_saved(root).len(), 1, "the other tab's edit is kept");
    assert_eq!(
        reviews_saved(root),
        [(pay_invoice(), Status::Validated, String::new())]
    );
}

// spec: SFM-15
#[tokio::test]
async fn a_refused_save_that_sent_no_text_answers_the_text_shown() {
    let dir = generated_repo();
    let root = dir.path();
    edit_name(root, &pay_invoice(), "Pay an invoice");

    let answer = post(root, FEATURE, "status=bogus", true).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert!(
        answer
            .body
            .contains("<input name=\"name\" value=\"Pay an invoice\">"),
        "{}",
        answer.body
    );
    assert!(
        answer.body.contains(">What pay-invoice does</textarea>"),
        "{}",
        answer.body
    );
}

// spec: SFM-3
#[tokio::test]
async fn a_name_a_browser_sends_without_its_line_breaks_is_unchanged() {
    let dir = generated_repo();
    let root = dir.path();
    edit_name(root, &pay_invoice(), "Pay\nan invoice");
    let before = both_files(root)[0].clone();

    post(
        root,
        FEATURE,
        "status=validated&name=Payan+invoice&description=What+pay-invoice+does",
        false,
    )
    .await;

    assert_eq!(both_files(root)[0], before);
}

// spec: SFM-18
#[tokio::test]
async fn the_form_says_under_its_statuses_what_each_one_means_once_saved() {
    let dir = generated_repo();
    let page = get(dir.path(), USE_CASE).await;
    let block = form_block(&page);
    let hint = block
        .split("<p class=\"hint status-hint\">")
        .nth(1)
        .and_then(|rest| rest.split("</p>").next())
        .unwrap_or_else(|| panic!("no hint under the statuses: {block}"));
    assert!(hint.contains("Validated"), "{hint}");
    assert!(hint.contains("writes it again"), "{hint}");
    assert!(hint.contains("To review"), "{hint}");
    assert!(
        block.find("status-hint") < block.find("name=\"comment\""),
        "the hint is under the statuses, before the comment: {block}"
    );

    let response = app(dir.path(), "fr")
        .oneshot(Request::get(USE_CASE).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let page = String::from_utf8(body.to_vec()).unwrap();
    assert!(page.contains("<p class=\"hint status-hint\">"), "{page}");
    assert!(page.contains("réécrit"), "{page}");
}
