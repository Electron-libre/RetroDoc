//! The outdated mark of an edit through the form of its section (`specs/section-form.md` SFM-3,
//! SFM-17, `specs/section-edit.md` SED-15): a field the reader leaves untouched stays outdated, and
//! only a confirmation brings the edit up to date, asked of the router without a socket.

mod support;

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_docs::edits::{self, Edit, Fields};
use retrodoc_docs::reviews::{Section, SectionKind};
use retrodoc_docs::GeneratedDocs;
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use support::{generated_repo, rewrite_run};
use tower::ServiceExt;

const FEATURE: &str = "/domains/billing/pay-invoice";

fn pay_invoice() -> Section {
    Section::new(SectionKind::Feature, "pay-invoice")
}

fn load(root: &Path) -> GeneratedDocs {
    GeneratedDocs::load(root, &[]).unwrap().unwrap()
}

/// The edit of `pay-invoice` saved in `root`.
fn saved(root: &Path) -> Edit {
    edits::load(root)
        .unwrap()
        .get(&pay_invoice())
        .unwrap()
        .clone()
}

/// The description of `pay-invoice` edited `OldD`, then rewritten by a later run: the edit is
/// outdated.
fn outdated_edit() -> tempfile::TempDir {
    let dir = generated_repo();
    let root = dir.path();
    let fields = Fields {
        description: Some("OldD".into()),
        ..Fields::default()
    };
    edits::record(root, &load(root), &pay_invoice(), fields).unwrap();
    rewrite_run(root);
    assert!(edits::outdated(&load(root), &saved(root)).is_some());
    dir
}

/// The browser of the served page posting `form` to `path`, with `htmx`; the status answered.
async fn post(root: &Path, path: &str, form: &str) -> StatusCode {
    let request = Request::post(path)
        .header("host", "127.0.0.1:8730")
        .header("origin", "http://127.0.0.1:8730")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true");
    let app = retrodoc_web::router(
        Project::default(),
        GeneratedDocs::load(root, &[]).unwrap(),
        DocLinks::default(),
        Vec::new(),
    );
    app.oneshot(request.body(Body::from(form.to_string())).unwrap())
        .await
        .unwrap()
        .status()
}

// spec: SFM-3, SED-15
#[tokio::test]
async fn correcting_one_field_of_an_outdated_edit_keeps_the_other_outdated() {
    let dir = outdated_edit();
    let root = dir.path();

    let status = post(root, FEATURE, "status=&name=Payments&description=OldD").await;

    assert_eq!(status, StatusCode::OK);
    let edit = saved(root);
    assert_eq!(edit.fields.name.as_deref(), Some("Payments"));
    assert_eq!(edit.fields.description.as_deref(), Some("OldD"));
    let outdated = edits::outdated(&load(root), &edit).expect("the description is still outdated");
    assert!(outdated.rewritten);
}
