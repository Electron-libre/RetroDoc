//! The reviews of the sections on their pages and in the lists of their parents, asked of the
//! router without a socket over the artifacts a run leaves in `.retrodoc/cache/` and the reviews
//! of `.retrodoc/reviews.json`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::artifacts::{DomainCluster, DomainMap};
use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
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

/// A run over `billing`: the features `pay-invoice` (the use cases `pay-by-card` and
/// `pay-by-cash`) and `refund`.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_core::artifacts::save_features(root, &[feature("pay-invoice"), feature("refund")])
        .unwrap();
    retrodoc_core::artifacts::save_use_cases(
        root,
        &[use_case("pay-by-card"), use_case("pay-by-cash")],
    )
    .unwrap();
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

fn load(root: &Path) -> GeneratedDocs {
    GeneratedDocs::load(root, &[]).unwrap().unwrap()
}

fn record(root: &Path, kind: SectionKind, id: &str, status: Status, comment: &str) {
    reviews::record(root, &load(root), &Section::new(kind, id), status, comment).unwrap();
}

async fn get(root: &Path, path: &str) -> (StatusCode, String) {
    get_in(root, path, Some("en")).await
}

async fn get_in(root: &Path, path: &str, lang: Option<&'static str>) -> (StatusCode, String) {
    let project = Project {
        name: "Shop".into(),
        lang,
        ..Project::default()
    };
    let response = retrodoc_web::router(project, Some(load(root)), DocLinks::default(), Vec::new())
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// The review block of a section page.
fn review_block(page: &str) -> &str {
    let start = page
        .find("<section class=\"review\"")
        .unwrap_or_else(|| panic!("no review block in {page}"));
    let end = start + page[start..].find("</section>").unwrap();
    &page[start..end]
}

/// The item of a list of the content that links to `href`: a row of the list of a page, else the card of a domain
/// on the home page.
fn row<'a>(page: &'a str, href: &str) -> &'a str {
    // After the tree of every page, which links to the same pages.
    let page = &page[page.find("<main").unwrap()..];
    let (start, close) = if let Some(at) = page.find(&format!("<li><a href=\"{href}\">")) {
        (at, "</li>")
    } else {
        let at = page
            .find(&format!("<h3><a href=\"{href}\">"))
            .unwrap_or_else(|| panic!("no row of {href} in {page}"));
        (page[..at].rfind("<article").unwrap(), "</article>")
    };
    let end = start + page[start..].find(close).unwrap();
    &page[start..end]
}

// spec: WEB-79
#[tokio::test]
async fn a_section_page_shows_its_review_after_its_confidence() {
    let dir = generated_repo();
    let root = dir.path();
    std::fs::write(
        reviews::reviews_path(root),
        r#"{"version": 1, "reviews": [{"kind": "feature", "id": "pay-invoice", "status": "wrong",
            "comment": "Misses the <b>refunds</b>", "author": "Ada",
            "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "x"}]}"#,
    )
    .unwrap();

    let (status, page) = get(root, "/domains/billing/pay-invoice").await;

    assert_eq!(status, StatusCode::OK);
    let block = review_block(&page);
    assert!(
        block.contains("class=\"badge review wrong\">Wrong</span>"),
        "{block}"
    );
    assert!(
        block.contains("Misses the &#60;b&#62;refunds&#60;/b&#62;"),
        "{block}"
    );
    assert!(block.contains("Ada"), "{block}");
    assert!(block.contains("2026-11-04T10:00:00Z"), "{block}");
    let confidence = page.find("class=\"score\"").unwrap();
    assert!(confidence < page.find("<section class=\"review\"").unwrap());
}

// spec: WEB-79
#[tokio::test]
async fn a_review_with_no_author_shows_its_status_comment_and_date() {
    let dir = generated_repo();
    let root = dir.path();
    // Not a git repository: no `user.name`.
    record(
        root,
        SectionKind::UseCase,
        "pay-invoice/pay-by-card",
        Status::Validated,
        "",
    );

    let (_, page) = get(root, "/domains/billing/pay-invoice/pay-by-card").await;

    let block = review_block(&page);
    assert!(block.contains(">Validated</span>"), "{block}");
    assert!(!block.contains("by "), "{block}");
    let saved = reviews::load(root).unwrap().reviews()[0]
        .reviewed_at
        .clone();
    assert!(block.contains(saved.as_str()), "{block}");
}

// spec: WEB-79
#[tokio::test]
async fn a_section_with_no_review_says_it_is_not_reviewed() {
    let dir = generated_repo();
    for path in [
        "/domains/billing",
        "/domains/billing/refund",
        "/domains/billing/pay-invoice/pay-by-cash",
    ] {
        let (_, page) = get(dir.path(), path).await;
        assert!(review_block(&page).contains("Not reviewed"), "{path}");
    }
}

// spec: WEB-80
#[tokio::test]
async fn the_rows_of_the_lists_carry_the_status_of_their_review() {
    let dir = generated_repo();
    let root = dir.path();
    record(root, SectionKind::Domain, "billing", Status::ToReview, "");
    record(root, SectionKind::Feature, "pay-invoice", Status::Wrong, "");
    record(
        root,
        SectionKind::UseCase,
        "pay-invoice/pay-by-card",
        Status::Validated,
        "",
    );

    let (_, home) = get(root, "/").await;
    let billing = row(&home, "/domains/billing");
    assert!(
        billing.contains("badge review to_review\">To review<"),
        "{billing}"
    );

    let (_, domain) = get(root, "/domains/billing").await;
    let pay = row(&domain, "/domains/billing/pay-invoice");
    assert!(pay.contains("badge review wrong\">Wrong<"), "{pay}");
    assert!(!row(&domain, "/domains/billing/refund").contains("badge review"));

    let (_, feature) = get(root, "/domains/billing/pay-invoice").await;
    let card = row(&feature, "/domains/billing/pay-invoice/pay-by-card");
    assert!(
        card.contains("badge review validated\">Validated<"),
        "{card}"
    );
    assert!(!row(&feature, "/domains/billing/pay-invoice/pay-by-cash").contains("badge review"));
}

// spec: WEB-82
#[tokio::test]
async fn a_review_saved_while_the_server_runs_shows_on_the_next_request() {
    let dir = generated_repo();
    let root = dir.path();
    let router = retrodoc_web::router(
        Project::default(),
        Some(load(root)),
        DocLinks::default(),
        Vec::new(),
    );
    let ask = |router: axum::Router| async move {
        let response = router
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
        String::from_utf8(body.to_vec()).unwrap()
    };
    assert!(review_block(&ask(router.clone()).await).contains("Not reviewed"));

    record(
        root,
        SectionKind::Domain,
        "billing",
        Status::Validated,
        "Checked",
    );

    let page = ask(router).await;
    assert!(review_block(&page).contains("Checked"), "{page}");
}

// spec: WEB-87
#[tokio::test]
async fn an_unreadable_reviews_file_is_named_in_place_of_the_review() {
    let dir = generated_repo();
    let root = dir.path();
    std::fs::write(reviews::reviews_path(root), "{ not json").unwrap();

    for path in [
        "/",
        "/domains/billing",
        "/domains/billing/pay-invoice",
        "/domains/billing/pay-invoice/pay-by-card",
    ] {
        let (status, page) = get(root, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        if path != "/" {
            let block = review_block(&page);
            assert!(block.contains("reviews.json cannot be read"), "{block}");
        }
    }
}

// spec: WEB-87
#[tokio::test]
async fn a_reviews_file_of_a_later_format_is_named_with_its_version() {
    let dir = generated_repo();
    let root = dir.path();
    std::fs::write(
        reviews::reviews_path(root),
        r#"{"version": 99, "reviews": []}"#,
    )
    .unwrap();

    let (_, page) = get(root, "/domains/billing").await;

    assert!(review_block(&page).contains("of format 99"), "{page}");
}

// spec: WEB-77, WEB-79
#[tokio::test]
async fn the_review_is_worded_in_french() {
    let dir = generated_repo();
    let root = dir.path();
    record(
        root,
        SectionKind::Feature,
        "pay-invoice",
        Status::ToReview,
        "",
    );

    let (_, page) = get_in(root, "/domains/billing/pay-invoice", Some("fr")).await;
    assert!(review_block(&page).contains(">À revoir</span>"), "{page}");
    let (_, page) = get_in(root, "/domains/billing/refund", Some("fr")).await;
    assert!(review_block(&page).contains("Non relu"), "{page}");
}

/// What a `POST` got: its status and its body.
struct Answer {
    status: StatusCode,
    body: String,
}

/// The browser of the served page posting `form` to `path`, sent by `htmx` when `htmx`.
async fn post(root: &Path, path: &str, form: &str, htmx: bool) -> Answer {
    let mut headers = vec![("origin", "http://127.0.0.1:8730")];
    if htmx {
        headers.push(("hx-request", "true"));
    }
    post_with(Some(load(root)), path, form, &headers).await
}

async fn post_with(
    docs: Option<GeneratedDocs>,
    path: &str,
    form: &str,
    headers: &[(&str, &str)],
) -> Answer {
    let mut request = Request::post(path)
        .header("host", "127.0.0.1:8730")
        .header("content-type", "application/x-www-form-urlencoded");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = retrodoc_web::router(Project::default(), docs, DocLinks::default(), Vec::new())
        .oneshot(request.body(Body::from(form.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    Answer {
        status,
        body: String::from_utf8(body.to_vec()).unwrap(),
    }
}

// spec: WEB-84
#[tokio::test]
async fn a_post_from_another_site_is_refused_and_saves_nothing() {
    let dir = generated_repo();
    let root = dir.path();
    let refused: [&[(&str, &str)]; 6] = [
        &[("origin", "http://evil.example")],
        &[("origin", "null")],
        &[("origin", "https://127.0.0.1:8730")],
        &[("origin", "http://127.0.0.1:8731")],
        &[("referer", "http://evil.example/domains/billing")],
        &[],
    ];
    for headers in refused {
        let answer = post_with(
            Some(load(root)),
            "/domains/billing",
            "status=wrong",
            headers,
        )
        .await;
        assert_eq!(answer.status, StatusCode::FORBIDDEN, "{headers:?}");
    }
    assert!(!reviews::reviews_path(root).exists());

    let answer = post_with(
        Some(load(root)),
        "/domains/billing",
        "status=wrong",
        &[("referer", "http://127.0.0.1:8730/domains/billing")],
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
}

// spec: WEB-84
#[tokio::test]
async fn an_origin_that_is_not_text_is_refused_though_the_referer_matches() {
    let dir = generated_repo();
    let root = dir.path();
    let request = Request::post("/domains/billing")
        .header("host", "127.0.0.1:8730")
        .header("content-type", "application/x-www-form-urlencoded")
        .header(
            "origin",
            axum::http::HeaderValue::from_bytes(b"http://\xff").unwrap(),
        )
        .header("referer", "http://127.0.0.1:8730/domains/billing")
        .body(Body::from("status=wrong"))
        .unwrap();

    let response = retrodoc_web::router(
        Project::default(),
        Some(load(root)),
        DocLinks::default(),
        Vec::new(),
    )
    .oneshot(request)
    .await
    .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(!reviews::reviews_path(root).exists());
}

/// The files of the repository and their bytes.
fn snapshot(root: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    fn walk(dir: &Path, out: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push((path.clone(), std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

// spec: WEB-12
#[tokio::test]
async fn a_review_sent_writes_the_reviews_file_alone() {
    let dir = generated_repo();
    let root = dir.path();
    let before = snapshot(root);

    post(root, "/domains/billing", "status=wrong", false).await;

    let reviews_file = reviews::reviews_path(root);
    let after: Vec<_> = snapshot(root)
        .into_iter()
        .filter(|(path, _)| *path != reviews_file)
        .collect();
    assert_eq!(after, before);
    assert!(reviews_file.exists());
}

/// The run of [`generated_repo`] whose feature `refund` has the source file `app/refund.rb`,
/// written in the repository.
fn repo_with_a_cited_file() -> tempfile::TempDir {
    let dir = generated_repo();
    let root = dir.path();
    let mut refund = feature("refund");
    refund.source_paths = vec!["app/refund.rb".into()];
    retrodoc_core::artifacts::save_features(root, &[feature("pay-invoice"), refund]).unwrap();
    std::fs::create_dir_all(root.join("app")).unwrap();
    std::fs::write(root.join("app/refund.rb"), "class Refund; end").unwrap();
    dir
}

// spec: WEB-89
#[tokio::test]
async fn the_review_of_a_section_says_it_is_outdated_and_why() {
    let dir = repo_with_a_cited_file();
    let root = dir.path();
    record(root, SectionKind::Feature, "refund", Status::Validated, "");
    record(root, SectionKind::Feature, "pay-invoice", Status::Wrong, "");
    let (_, page) = get(root, "/domains/billing/refund").await;
    assert!(!review_block(&page).contains("Outdated review"), "{page}");

    std::fs::write(
        root.join("app/refund.rb"),
        "class Refund; def undo; end; end",
    )
    .unwrap();
    let mut rewritten = feature("pay-invoice");
    rewritten.description = "Pays an invoice by card or cash".into();
    let mut refund = feature("refund");
    refund.source_paths = vec!["app/refund.rb".into()];
    retrodoc_core::artifacts::save_features(root, &[rewritten, refund]).unwrap();

    let (_, page) = get(root, "/domains/billing/refund").await;
    let block = review_block(&page);
    assert!(
        block.contains(
            "Outdated review: files changed since the review: <code>app/refund.rb</code> (modified)"
        ),
        "{block}"
    );
    let (_, page) = get(root, "/domains/billing/pay-invoice").await;
    let block = review_block(&page);
    assert!(
        block.contains("Outdated review: section rewritten since the review</span>"),
        "{block}"
    );
}

// spec: WEB-89
#[tokio::test]
async fn saving_an_outdated_review_again_makes_it_up_to_date() {
    let dir = repo_with_a_cited_file();
    let root = dir.path();
    record(root, SectionKind::Feature, "refund", Status::Validated, "");
    std::fs::write(
        root.join("app/refund.rb"),
        "class Refund; def undo; end; end",
    )
    .unwrap();
    let (_, page) = get(root, "/domains/billing/refund").await;
    assert!(review_block(&page).contains("Outdated review"), "{page}");

    let answer = post(
        root,
        "/domains/billing/refund",
        "status=validated&comment=",
        true,
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(!answer.body.contains("Outdated review"), "{}", answer.body);
    let (_, page) = get(root, "/domains/billing/refund").await;
    assert!(!review_block(&page).contains("Outdated review"), "{page}");
}

// spec: WEB-90
#[tokio::test]
async fn the_badge_of_an_outdated_review_in_a_list_says_so() {
    let dir = repo_with_a_cited_file();
    let root = dir.path();
    record(root, SectionKind::Domain, "billing", Status::Validated, "");
    record(root, SectionKind::Feature, "refund", Status::Validated, "");
    record(root, SectionKind::Feature, "pay-invoice", Status::Wrong, "");
    record(
        root,
        SectionKind::UseCase,
        "pay-invoice/pay-by-card",
        Status::Validated,
        "",
    );
    std::fs::write(root.join("app/refund.rb"), "edited").unwrap();
    let mut card = use_case("pay-by-card");
    card.description = "Pays by card".into();
    retrodoc_core::artifacts::save_use_cases(root, &[card, use_case("pay-by-cash")]).unwrap();

    let outdated = "<span class=\"badge review outdated\">outdated</span>";
    let (_, home) = get(root, "/").await;
    // The domain cites the files of its features.
    let billing = row(&home, "/domains/billing");
    assert!(billing.contains(outdated), "{billing}");
    let (_, domain) = get(root, "/domains/billing").await;
    let refund = row(&domain, "/domains/billing/refund");
    assert!(refund.contains(outdated), "{refund}");
    let pay = row(&domain, "/domains/billing/pay-invoice");
    assert!(!pay.contains(outdated), "{pay}");
    let (_, feature) = get(root, "/domains/billing/pay-invoice").await;
    let card = row(&feature, "/domains/billing/pay-invoice/pay-by-card");
    assert!(card.contains(outdated), "{card}");
}
