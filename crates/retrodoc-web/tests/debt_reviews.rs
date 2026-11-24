//! What the human reviews change in the debt page and the audit page, asked of the router without
//! a socket over the artifacts a run leaves and the reviews saved in
//! `.retrodoc/reviews.json`.

mod support;

use axum::body::Body;
use axum::http::Request;
use retrodoc_docs::reviews::{self, SectionKind, Status};
use retrodoc_docs::GeneratedDocs;
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use support::*;
use tower::ServiceExt;

// spec: DBT-32
#[tokio::test]
async fn a_validated_section_leaves_the_kinds_of_judgement_only() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/pay-by-card",
        Status::Validated,
        "",
    );
    let page = get(repo.path(), "/debt").await;
    // Still set apart: the docs leave it out by design, whatever a human said.
    assert_eq!(
        listed(&page),
        [pair("Set apart", "/domains/billing/pay-invoice/refund")],
        "{page}"
    );
}

// spec: DBT-32
#[tokio::test]
async fn a_validated_section_rewritten_since_is_listed_again() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    let mut rewritten = use_cases();
    rewritten[0].description = "Refunds the paid invoice.".into();
    retrodoc_core::artifacts::save_use_cases(repo.path(), &rewritten).unwrap();
    let page = get(repo.path(), "/debt").await;
    assert!(
        listed(&page).contains(&pair(
            "Low confidence",
            "/domains/billing/pay-invoice/refund"
        )),
        "{page}"
    );
}

// spec: DBT-32
#[tokio::test]
async fn a_validated_section_whose_cited_file_changed_is_listed_again() {
    let repo = generated_repo();
    let root = repo.path();
    std::fs::create_dir_all(root.join("app")).unwrap();
    std::fs::write(root.join("app/pay-invoice.rb"), "class PayInvoice; end").unwrap();
    review(
        root,
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    let refund = pair("Low confidence", "/domains/billing/pay-invoice/refund");
    assert!(!listed(&get(root, "/debt").await).contains(&refund));

    std::fs::write(
        root.join("app/pay-invoice.rb"),
        "class PayInvoice; def refund; end; end",
    )
    .unwrap();

    let page = get(root, "/debt").await;
    assert!(listed(&page).contains(&refund), "{page}");
}

// spec: DBT-37
#[tokio::test]
async fn an_item_marked_wrong_or_to_review_says_its_review_is_outdated() {
    let repo = generated_repo();
    let root = repo.path();
    std::fs::create_dir_all(root.join("app")).unwrap();
    std::fs::write(root.join("app/ship-order.rb"), "class ShipOrder; end").unwrap();
    review(root, SectionKind::Feature, "ship-order", Status::Wrong, "");
    review(
        root,
        SectionKind::UseCase,
        "pay-invoice/pay-by-card",
        Status::ToReview,
        "",
    );
    let page = get(root, "/debt").await;
    assert!(!page.contains("Outdated review"), "{page}");

    std::fs::write(
        root.join("app/ship-order.rb"),
        "class ShipOrder; def ship; end; end",
    )
    .unwrap();
    let mut rewritten = use_cases();
    rewritten[1].description = "Pays the invoice by card.".into();
    retrodoc_core::artifacts::save_use_cases(root, &rewritten).unwrap();

    let page = get(root, "/debt").await;
    let item = |kind: &str| {
        items(&page)
            .into_iter()
            .find(|item| item.contains(&format!("class=\"badge kind\">{kind}</span>")))
            .unwrap_or_else(|| panic!("no {kind} in {page}"))
    };
    let wrong = item("Marked wrong");
    assert!(
        wrong.contains(
            "Outdated review: files changed since the review: <code>app/ship-order.rb</code> (modified)"
        ),
        "{wrong}"
    );
    assert!(!wrong.contains("rewritten"), "{wrong}");
    let to_review = item("To review");
    assert!(
        to_review.contains("Outdated review: section rewritten since the review"),
        "{to_review}"
    );
}

// spec: DBT-30, DBT-31, DBT-33, DBT-29, DBT-8
#[tokio::test]
async fn a_section_marked_wrong_or_to_review_enters_the_debt_with_its_comment() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::Domain,
        "billing",
        Status::Wrong,
        "Refunds are <b>not</b> billing",
    );
    review(
        repo.path(),
        SectionKind::Feature,
        "ship-order",
        Status::ToReview,
        "",
    );
    review(
        repo.path(),
        SectionKind::UseCase,
        "ship-order/ship",
        Status::Wrong,
        "",
    );
    // An up-to-date review settles its section (FIX-1): only an outdated one is debt.
    rewrite_run(repo.path());
    let page = get(repo.path(), "/debt").await;
    assert_eq!(
        listed(&page),
        [
            // Weight 1 and the entry point of its feature: ahead of the use case, which has none.
            pair("Marked wrong", "/domains/billing"),
            pair("Marked wrong", "/domains/shipping/ship-order/ship"),
            // Weight 0.5 and one entry point, against weight 1 and weakness 0.7.
            pair("To review", "/domains/shipping/ship-order"),
            pair("Low confidence", "/domains/billing/pay-invoice/refund"),
            pair("Not scored", "/domains/billing/pay-invoice/pay-by-card"),
            pair("Set apart", "/domains/billing/pay-invoice/refund"),
        ],
        "{page}"
    );
    let items = items(&page);
    assert!(items[0].contains("Billing"), "{}", items[0]);
    assert!(
        items[0].contains("Refunds are &#60;b&#62;not&#60;/b&#62; billing"),
        "{}",
        items[0]
    );
    assert!(
        items[0].contains("weight 1.00, weakness 1.00, 0 commits, 1 entry point"),
        "{}",
        items[0]
    );
    assert!(items[2].contains("weight 0.50"), "{}", items[2]);
    assert!(!items[2].contains("class=\"why\""), "{}", items[2]);
    let filtered = get(repo.path(), "/debt?kind=to_review&domain=shipping").await;
    assert_eq!(
        listed(&filtered),
        [pair("To review", "/domains/shipping/ship-order")]
    );
}

// spec: DBT-30
#[tokio::test]
async fn a_review_of_a_section_the_run_no_longer_holds_is_not_listed() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::Feature,
        "ship-order",
        Status::Wrong,
        "",
    );
    retrodoc_core::artifacts::save_features(
        repo.path(),
        &[feature("pay-invoice", "billing", "Pay an invoice")],
    )
    .unwrap();
    let page = get(repo.path(), "/debt").await;
    assert!(!page.contains("Marked wrong</span>"), "{page}");
}

// spec: DBT-34
#[tokio::test]
async fn an_unreadable_file_of_reviews_is_said_and_the_debt_listed_without_it() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    let page = get(repo.path(), "/debt").await;
    let low = "class=\"badge kind\">Low confidence</span>";
    assert!(!page.contains(low), "{page}");
    std::fs::write(reviews::reviews_path(repo.path()), "{ not json").unwrap();
    let page = get(repo.path(), "/debt").await;
    assert!(page.contains("The reviews could not be read"), "{page}");
    assert!(page.contains(low), "{page}");
}

// spec: DBT-34
#[tokio::test]
async fn a_review_saved_while_the_server_runs_shows_on_the_next_debt_page() {
    let repo = generated_repo();
    let app = retrodoc_web::router(
        Project::default(),
        GeneratedDocs::load(repo.path(), &[]).unwrap(),
        DocLinks::default(),
        Vec::new(),
    );
    review(
        repo.path(),
        SectionKind::Feature,
        "ship-order",
        Status::ToReview,
        "",
    );
    let response = app
        .oneshot(Request::get("/debt").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&body).contains("To review</span>"));
}

// spec: AUD-6
#[tokio::test]
async fn the_heatmap_counts_the_sections_marked_wrong_and_to_review() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::Domain,
        "billing",
        Status::Wrong,
        "",
    );
    review(
        repo.path(),
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::ToReview,
        "",
    );
    // An up-to-date review settles its section (FIX-1): only an outdated one is debt.
    rewrite_run(repo.path());
    let page = get(repo.path(), "/audit").await;
    let heatmap = &page[page.find("id=\"audit-heatmap\"").expect(&page)..];
    let header = heatmap.split("</thead>").next().unwrap();
    let undocumented = header.find("Undocumented").expect(header);
    let wrong = header.find("Marked wrong").expect(header);
    let to_review = header.find("To review").expect(header);
    assert!(undocumented < wrong && wrong < to_review, "{header}");
    assert!(
        heatmap.contains("href=\"/debt?kind=marked_wrong&#38;domain=billing\""),
        "{heatmap}"
    );
    assert!(
        heatmap.contains("href=\"/debt?kind=to_review&#38;domain=billing\""),
        "{heatmap}"
    );
}

/// The text of the figures of the reviews on the audit page.
fn reviewed(page: &str) -> String {
    let start = page.find("id=\"audit-reviews\"").expect(page);
    let section = &page[start..];
    section[..section.find("</section>").unwrap()].to_string()
}

// spec: AUD-16
#[tokio::test]
async fn the_audit_page_shows_the_share_of_each_kind_of_section_reviewed_by_status() {
    let repo = generated_repo();
    let root = repo.path();
    review(root, SectionKind::Domain, "billing", Status::Wrong, "");
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
    review(
        root,
        SectionKind::UseCase,
        "ship-order/ship",
        Status::ToReview,
        "",
    );
    // The run no longer holds `ship-order` nor its domain: neither its review nor its use case
    // count.
    retrodoc_core::artifacts::save_features(
        root,
        &[feature("pay-invoice", "billing", "Pay an invoice")],
    )
    .unwrap();

    let reviewed = reviewed(&get(root, "/audit").await);

    for line in [
        "<strong>100%</strong> <span class=\"muted\">Domains reviewed: 1 of 1 · Validated: 0, Wrong: 1, To review: 0 · Outdated: 0</span>",
        "<strong>0%</strong> <span class=\"muted\">Features reviewed: 0 of 1 · Validated: 0, Wrong: 0, To review: 0 · Outdated: 0</span>",
        "<strong>50%</strong> <span class=\"muted\">Use cases reviewed: 1 of 2 · Validated: 1, Wrong: 0, To review: 0 · Outdated: 0</span>",
    ] {
        assert!(reviewed.contains(line), "{line} not in {reviewed}");
    }
}

// spec: AUD-18
#[tokio::test]
async fn the_audit_page_counts_the_outdated_reviews_of_each_kind() {
    let repo = generated_repo();
    let root = repo.path();
    review(root, SectionKind::Domain, "billing", Status::Wrong, "");
    review(
        root,
        SectionKind::UseCase,
        "pay-invoice/refund",
        Status::Validated,
        "",
    );
    review(
        root,
        SectionKind::UseCase,
        "ship-order/ship",
        Status::Validated,
        "",
    );
    let mut rewritten = use_cases();
    rewritten[0].description = "Refunds the paid invoice.".into();
    retrodoc_core::artifacts::save_use_cases(root, &rewritten).unwrap();

    let reviewed = reviewed(&get(root, "/audit").await);

    for line in [
        "Domains reviewed: 1 of 2 · Validated: 0, Wrong: 1, To review: 0 · Outdated: 0</span>",
        "Use cases reviewed: 2 of 3 · Validated: 2, Wrong: 0, To review: 0 · Outdated: 1</span>",
    ] {
        assert!(reviewed.contains(line), "{line} not in {reviewed}");
    }
}

// spec: AUD-17
#[tokio::test]
async fn an_unreadable_file_of_reviews_is_said_on_the_audit_page() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::Domain,
        "billing",
        Status::Wrong,
        "",
    );
    std::fs::write(reviews::reviews_path(repo.path()), "{ not json").unwrap();

    let page = get(repo.path(), "/audit").await;

    let reviewed = reviewed(&page);
    assert!(
        reviewed.contains("Missing: the reviews could not be read"),
        "{reviewed}"
    );
    assert!(!reviewed.contains("Domains reviewed"), "{reviewed}");
    assert!(!page.contains("kind=marked_wrong"), "{page}");
    assert!(page.contains("id=\"audit-heatmap\""), "{page}");
}

// spec: DBT-39, DBT-29
#[tokio::test]
async fn an_edit_of_a_section_rewritten_since_enters_the_debt() {
    let repo = generated_repo();
    let root = repo.path();
    edit_name(
        root,
        SectionKind::Feature,
        "ship-order",
        "Dispatch an order",
    );
    edit_name(
        root,
        SectionKind::UseCase,
        "pay-invoice/pay-by-card",
        "Card",
    );
    let page = get(root, "/debt").await;
    assert!(!page.contains("kind\">Outdated edit</span>"), "{page}");

    let mut rewritten = feature("ship-order", "shipping", "Ship an order");
    rewritten.description = "Sends the parcel.".into();
    retrodoc_core::artifacts::save_features(
        root,
        &[
            feature("pay-invoice", "billing", "Pay an invoice"),
            rewritten,
        ],
    )
    .unwrap();
    let mut use_cases = use_cases();
    use_cases.retain(|u| u.slug != "pay-by-card");
    retrodoc_core::artifacts::save_use_cases(root, &use_cases).unwrap();

    let page = get(root, "/debt").await;
    let edited: Vec<String> = items(&page)
        .into_iter()
        .filter(|item| item.contains("class=\"badge kind\">Outdated edit</span>"))
        .collect();
    assert_eq!(edited.len(), 1, "{page}");
    assert!(
        edited[0].contains("/domains/shipping/ship-order\""),
        "{}",
        edited[0]
    );
    assert!(
        edited[0].contains("Outdated edit: section rewritten since the edit"),
        "{}",
        edited[0]
    );
    assert!(edited[0].contains("weight 0.50"), "{}", edited[0]);
}

// spec: DBT-H1, DBT-1, DBT-9, DBT-23, DBT-31
#[tokio::test]
async fn the_auditor_sees_the_debt_ranked_and_filters_it_by_kind() {
    let repo = generated_repo();
    review(
        repo.path(),
        SectionKind::Feature,
        "ship-order",
        Status::ToReview,
        "",
    );

    let page = get(repo.path(), "/debt").await;

    assert_eq!(
        listed(&page),
        [
            pair("To review", "/domains/shipping/ship-order"),
            pair("Low confidence", "/domains/billing/pay-invoice/refund"),
            pair("Not scored", "/domains/billing/pay-invoice/pay-by-card"),
            pair("Set apart", "/domains/billing/pay-invoice/refund"),
        ],
        "{page}"
    );
    assert!(items(&page)[1].contains("Billing"), "{}", items(&page)[1]);
    let filtered = get(repo.path(), "/debt?kind=to_review").await;
    assert_eq!(
        listed(&filtered),
        [pair("To review", "/domains/shipping/ship-order")]
    );
}
