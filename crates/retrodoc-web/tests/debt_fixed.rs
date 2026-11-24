//! The sections a human settled, listed apart under "Fixed" on the debt page
//! (`specs/debt-fixed.md`), asked of the router without a socket.

mod support;

use axum::body::Body;
use axum::http::Request;
use retrodoc_docs::reviews::{self, SectionKind, Status};
use retrodoc_docs::GeneratedDocs;
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use support::*;
use tower::ServiceExt;

/// The "Fixed" part of the debt list, empty when the page shows none.
fn fixed(page: &str) -> String {
    page.find("id=\"debt-fixed\"")
        .map_or_else(String::new, |start| {
            let part = &page[start..];
            part[..part.find("</div>").expect(part)].to_string()
        })
}

/// The rows of the "Fixed" part, one string per `<li class="fixed-item">`.
fn fixed_rows(page: &str) -> Vec<String> {
    fixed(page)
        .split("<li class=\"fixed-item\"")
        .skip(1)
        .map(str::to_string)
        .collect()
}

/// The kinds of the items left, in order.
fn kinds_left(page: &str) -> Vec<String> {
    let start = page.find("id=\"debt-list\"").expect(page);
    let end = page.find("id=\"debt-fixed\"").unwrap_or(page.len());
    page[start..end]
        .split("class=\"badge kind\">")
        .skip(1)
        .map(|rest| rest[..rest.find("</span>").unwrap()].to_string())
        .collect()
}

// spec: FIX-H1, FIX-1, FIX-2, FIX-3, FIX-4, FIX-6
#[tokio::test]
async fn a_use_case_marked_wrong_leaves_the_items_left_for_fixed() {
    let repo = generated_repo();
    let root = repo.path();
    assert!(get(root, "/").await.contains("Review the 3 items"));

    review(
        root,
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Wrong,
        "Wrong <b>actor</b>",
    );

    let page = get(root, "/debt").await;
    assert_eq!(kinds_left(&page), ["Not scored", "Set apart"], "{page}");
    let rows = fixed_rows(&page);
    assert_eq!(rows.len(), 1, "{page}");
    let row = &rows[0];
    assert!(
        row.contains("class=\"badge review wrong\">Wrong</span>"),
        "{row}"
    );
    assert!(
        row.contains("href=\"/domains/billing/pay-invoice/refund\""),
        "{row}"
    );
    assert!(row.contains("Wrong &#60;b&#62;actor&#60;/b&#62;"), "{row}");
    let saved = reviews::load(root).unwrap();
    assert!(row.contains(&saved.reviews()[0].reviewed_at), "{row}");
    assert!(
        row.contains("Took out: Low confidence, Marked wrong"),
        "{row}"
    );
    assert!(get(root, "/").await.contains("Review the 2 items"));
}

// spec: FIX-H2, FIX-3
#[tokio::test]
async fn a_feature_marked_wrong_then_edited_shows_its_edit() {
    let repo = generated_repo();
    let root = repo.path();
    review(root, SectionKind::Feature, "ship-order", Status::Wrong, "");
    assert!(!fixed(&get(root, "/debt").await).contains("Edited"));

    edit_name(
        root,
        SectionKind::Feature,
        "ship-order",
        "Dispatch an order",
    );

    let page = get(root, "/debt").await;
    let rows = fixed_rows(&page);
    assert_eq!(rows.len(), 1, "{page}");
    assert!(rows[0].contains("Dispatch an order"), "{}", rows[0]);
    assert!(rows[0].contains("Edited"), "{}", rows[0]);
    assert!(
        !kinds_left(&page).contains(&"Marked wrong".to_string()),
        "{page}"
    );
}

// spec: FIX-H3, FIX-3, FIX-4
#[tokio::test]
async fn a_validated_use_case_shows_the_kind_it_took_out() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );

    let rows = fixed_rows(&get(repo.path(), "/debt").await);

    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].contains("class=\"badge review validated\">Validated</span>"),
        "{}",
        rows[0]
    );
    assert!(rows[0].contains("Took out: Low confidence<"), "{}", rows[0]);
    assert!(!rows[0].contains("class=\"why\""), "{}", rows[0]);
}

// spec: FIX-8, FIX-9
#[tokio::test]
async fn the_filters_keep_the_fixed_sections_of_their_domain_and_kind() {
    let repo = generated_repo();
    let root = repo.path();
    review(
        root,
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    review(
        root,
        SectionKind::Feature,
        "ship-order",
        Status::Validated,
        "",
    );

    assert_eq!(fixed_rows(&get(root, "/debt").await).len(), 2);
    let billing = fixed_rows(&get(root, "/debt?domain=billing").await);
    assert_eq!(billing.len(), 1);
    assert!(billing[0].contains("Refund"), "{}", billing[0]);
    let low = fixed_rows(&get(root, "/debt?kind=low_confidence").await);
    assert_eq!(low.len(), 1);
    assert!(low[0].contains("Refund"), "{}", low[0]);
    let stale = get(root, "/debt?kind=stale").await;
    assert_eq!(fixed(&stale), "", "{stale}");
    assert!(!stale.contains(">Fixed</h2>"), "{stale}");
}

// spec: FIX-9
#[tokio::test]
async fn with_no_review_the_page_shows_no_fixed_heading() {
    let repo = generated_repo();

    let page = get(repo.path(), "/debt").await;

    assert!(!page.contains("id=\"debt-fixed\""), "{page}");
}

// spec: FIX-10
#[tokio::test]
async fn the_list_htmx_swaps_holds_the_fixed_sections() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    let response = retrodoc_web::router(
        Project::default(),
        GeneratedDocs::load(repo.path(), &[]).unwrap(),
        DocLinks::default(),
        Vec::new(),
    )
    .oneshot(
        Request::get("/debt")
            .header("HX-Request", "true")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let fragment = String::from_utf8(body.to_vec()).unwrap();

    assert!(!fragment.contains("<html"), "{fragment}");
    assert_eq!(fixed_rows(&fragment).len(), 1, "{fragment}");
}
