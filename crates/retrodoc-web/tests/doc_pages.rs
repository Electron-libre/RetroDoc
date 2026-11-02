//! The pages of the collected Markdown docs, `/docs/<path>`, asked of the
//! router without a socket.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_mcp::ExistingDoc;
use retrodoc_render::{anchor, DocLinks};
use tower::ServiceExt;

const GUIDE: &str = "\
# Guide

Intro with <b>bold</b> raw HTML.

<script>alert(1)</script>

## Paying **now** `card`

See [the other doc](other.md), [the site](https://example.com) and [mail](mailto:a@example.com).

Click [here](javascript:alert(2)) or [there]( JavaScript:alert(3)).

![logo](data:image/png;base64,AAAA) ![chart](img/chart.png)

## Paying **now** `card`
";

fn collected() -> Vec<ExistingDoc> {
    vec![
        ExistingDoc {
            path: PathBuf::from("docs/guide.md"),
            content: GUIDE.into(),
        },
        ExistingDoc {
            path: PathBuf::from("README.md"),
            content: "No heading at all.".into(),
        },
    ]
}

async fn get(path: &str) -> (StatusCode, String) {
    // No run: the collected docs are served all the same.
    let response = retrodoc_web::router(
        retrodoc_web::Project::default(),
        None,
        DocLinks::default(),
        collected(),
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

/// The rendered doc, without the layout around it.
fn article(page: &str) -> &str {
    let start = page.find("<article").expect(page);
    &page[start..start + page[start..].find("</article>").unwrap()]
}

// spec: WEB-47
#[tokio::test]
async fn a_collected_doc_is_rendered_as_html_even_with_no_run() {
    let (status, page) = get("/docs/docs/guide.md").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let doc = article(&page);
    assert!(doc.contains("<h1 id=\"guide\">Guide</h1>"), "{doc}");
    assert!(doc.contains("<strong>now</strong>"), "{doc}");
    assert!(page.contains("<code>docs/guide.md</code>"), "{page}");
    let (status, page) = get("/docs/README.md").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        article(&page).contains("<p>No heading at all.</p>"),
        "{page}"
    );
}

// spec: WEB-48
#[tokio::test]
async fn each_heading_carries_the_anchor_the_doc_links_point_to() {
    let (_, page) = get("/docs/docs/guide.md").await;
    let id = format!("id=\"{}\"", anchor("Paying **now** `card`"));
    assert_eq!(article(&page).matches(&id).count(), 2, "{page}");
}

// spec: WEB-49
#[tokio::test]
async fn the_raw_html_of_a_doc_is_shown_as_text() {
    let (_, page) = get("/docs/docs/guide.md").await;
    let doc = article(&page);
    assert!(!doc.contains("<b>"), "{doc}");
    assert!(!doc.contains("<script>"), "{doc}");
    assert!(doc.contains("&lt;b&gt;bold&lt;/b&gt;"), "{doc}");
    assert!(
        doc.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
        "{doc}"
    );
}

// spec: WEB-50
#[tokio::test]
async fn a_link_with_an_unsafe_scheme_is_shown_as_its_text() {
    let (_, page) = get("/docs/docs/guide.md").await;
    let doc = article(&page);
    assert!(!doc.to_lowercase().contains("javascript:"), "{doc}");
    assert!(!doc.contains("data:"), "{doc}");
    assert!(doc.contains("Click here or there."), "{doc}");
    assert!(doc.contains("logo"), "{doc}");
    for kept in [
        "href=\"other.md\"",
        "href=\"https://example.com\"",
        "href=\"mailto:a@example.com\"",
        "src=\"img/chart.png\"",
    ] {
        assert!(doc.contains(kept), "{kept} missing: {doc}");
    }
}

// spec: WEB-51
#[tokio::test]
async fn a_path_that_is_not_a_collected_doc_is_not_found() {
    for path in ["/docs/docs/missing.md", "/docs/docs", "/docs/src/main.rs"] {
        let (status, page) = get(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(page.contains("<h1>Not found</h1>"), "{page}");
    }
}

// spec: WEB-47
#[tokio::test]
async fn the_front_matter_and_an_empty_heading_are_not_the_title() {
    let docs = vec![ExistingDoc {
        path: PathBuf::from("docs/setup.md"),
        content: "---\ntitle: Old\nlayout: page\n---\n#\n\n## Setup\n\nRun it.\n".into(),
    }];
    let project = retrodoc_web::Project {
        name: "Shop".into(),
        ..retrodoc_web::Project::default()
    };
    let response = retrodoc_web::router(project, None, DocLinks::default(), docs)
        .oneshot(
            Request::get("/docs/docs/setup.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let page = String::from_utf8(body.to_vec()).unwrap();
    assert!(page.contains("<title>Setup · Shop</title>"), "{page}");
    assert!(!article(&page).contains("layout"), "{page}");
}
