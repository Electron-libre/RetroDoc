//! The settling of a disputed section from its form (`specs/section-settle.md`), asked of the
//! router without a socket over the artifacts a run leaves in `.retrodoc/cache/`, the model the
//! fake provider of the tests.

use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::request::Builder as RequestBuilder;
use axum::http::{header, Request, Response, StatusCode};
use axum::Router;
use retrodoc_core::artifacts::{DomainCluster, DomainMap};
use retrodoc_core::model::Feature;
use retrodoc_core::reconcile::{
    Contradiction, ItemKind, ItemVerdict, Reconciliation, SectionRef, Status,
};
use retrodoc_docs::{GeneratedDocs, Uncovered};
use retrodoc_llm::{LlmProvider, Role};
use retrodoc_pipeline::testing::{FakeLlm, FakeTurn};
use retrodoc_render::DocLinks;
use retrodoc_web::{Asking, Project};
use tower::ServiceExt;

/// A run over `orders`: the feature `list-orders`, which a section of `README.md` contradicts.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("app")).unwrap();
    std::fs::write(root.join("app/order.rb"), "class Order; end\n").unwrap();
    let feature = Feature {
        back_office: false,
        technical: false,
        slug: "list-orders".into(),
        domain_slug: "orders".into(),
        name: "List orders".into(),
        description: "The clerk sees the orders".into(),
        source_paths: vec!["app/order.rb".into()],
        entry_points: Vec::new(),
        confidence: None,
    };
    retrodoc_core::artifacts::save_features(root, &[feature]).unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "orders".into(),
            name: "Orders".into(),
            description: "Orders of the shop".into(),
            ..DomainCluster::default()
        }],
    }
    .save(root)
    .unwrap();
    dir
}

/// The docs of [`generated_repo`], `README.md#Orders` contradicting `orders/list-orders`.
fn docs(root: &Path) -> GeneratedDocs {
    let verdict = ItemVerdict {
        kind: ItemKind::Feature,
        label: "orders/list-orders".into(),
        domain: "orders".into(),
        name: "List orders".into(),
        entry_points: 1,
        confidence: None,
        status: Status::Contradicted,
        sections: Vec::new(),
        contradictions: vec![Contradiction {
            section: SectionRef::parse("README.md#Orders"),
            doc_states: "The customer sees their orders".into(),
            item_states: "The clerk sees the orders".into(),
        }],
    };
    GeneratedDocs::load(root, &[])
        .unwrap()
        .unwrap()
        .with_uncovered(Uncovered {
            reconciliation: Some(Reconciliation {
                items: vec![verdict],
            }),
            ..Uncovered::default()
        })
}

fn router(root: &Path, llm: Option<Arc<dyn LlmProvider>>) -> Router {
    let asking = llm.map(|llm| Asking { llm, max_rounds: 8 });
    retrodoc_web::router_with(
        Project {
            name: "Shop".into(),
            lang: Some("en"),
            ..Project::default()
        },
        Some(docs(root)),
        DocLinks::default(),
        Vec::new(),
        asking,
    )
}

fn get_request(path: &str, cookie: Option<&str>) -> Request<Body> {
    with_cookie(Request::get(path), cookie, false)
        .body(Body::empty())
        .unwrap()
}

fn post_request(path: &str, form: &str, cookie: Option<&str>, htmx: bool) -> Request<Body> {
    let request = Request::post(path)
        .header(header::ORIGIN, "http://127.0.0.1:8730")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    with_cookie(request, cookie, htmx)
        .body(Body::from(form.to_string()))
        .unwrap()
}

fn with_cookie(request: RequestBuilder, cookie: Option<&str>, htmx: bool) -> RequestBuilder {
    let mut request = request.header(header::HOST, "127.0.0.1:8730");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    if htmx {
        request = request.header("HX-Request", "true");
    }
    request
}

async fn send(router: &Router, request: Request<Body>) -> Response<Body> {
    router.clone().oneshot(request).await.unwrap()
}

async fn body(response: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn session_cookie(response: &Response<Body>) -> Option<String> {
    let set = response.headers().get(header::SET_COOKIE)?.to_str().ok()?;
    set.split(';').next().map(str::to_string)
}

/// Asks a question in the session of `cookie` (a new one without), waits for its end, and gives
/// the session cookie.
async fn ask(router: &Router, cookie: Option<&str>) -> String {
    let response = send(
        router,
        post_request("/ask", "question=Who+lists%3F", cookie, true),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = session_cookie(&response)
        .or_else(|| cookie.map(str::to_string))
        .unwrap();
    let number = body(response)
        .await
        .split("/ask/questions/")
        .nth(1)
        .and_then(|rest| rest.split('/').next().map(str::to_string))
        .unwrap();
    let ended = send(
        router,
        get_request(&format!("/ask/questions/{number}"), Some(&cookie)),
    )
    .await;
    assert_eq!(ended.status(), StatusCode::OK);
    cookie
}

/// Starts the settling of `form` with htmx, and gives the answer.
async fn settle(router: &Router, form: &str, cookie: Option<&str>) -> Response<Body> {
    send(router, post_request("/ask/settle", form, cookie, true)).await
}

/// The settle form of a section page, if any.
fn settle_form(page: &str) -> Option<&str> {
    let at = page.find("action=\"/ask/settle\"")?;
    let from = page[..at].rfind("<form")?;
    let to = at + page[at..].find("</form>")?;
    Some(&page[from..to])
}

const FEATURE: &str = "kind=feature&id=list-orders";

// spec: STL-1, STL-12
#[tokio::test]
async fn the_form_of_a_section_offers_a_settling_with_ask_only_and_confirms_over_exchanges() {
    let dir = generated_repo();
    let page = body(
        send(
            &router(dir.path(), None),
            get_request("/domains/orders/list-orders", None),
        )
        .await,
    )
    .await;
    assert!(settle_form(&page).is_none(), "{page}");

    let llm = Arc::new(FakeLlm::answering("The clerk."));
    let router = router(dir.path(), Some(llm));
    let page = body(send(&router, get_request("/domains/orders/list-orders", None)).await).await;
    let form = settle_form(&page).expect("a settle form");
    assert!(form.contains("name=\"kind\" value=\"feature\""), "{form}");
    assert!(form.contains("name=\"id\" value=\"list-orders\""), "{form}");
    assert!(form.contains("Settle with the agent"), "{form}");
    assert!(!form.contains("data-confirm"), "{form}");
    let review = &page[page.find("id=\"review\"").unwrap()..];
    assert!(
        review.contains("action=\"/ask/settle\""),
        "the form is in the review"
    );

    let cookie = ask(&router, None).await;
    let page = body(
        send(
            &router,
            get_request("/domains/orders/list-orders", Some(&cookie)),
        )
        .await,
    )
    .await;
    let form = settle_form(&page).unwrap();
    assert!(form.contains("data-confirm=\""), "{form}");
}

// spec: STL-2, STL-3, STL-4, STL-10
#[tokio::test]
async fn a_settling_empties_the_session_and_asks_the_section_with_its_contradictions() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::scripted(vec![
        FakeTurn::Text("Listed.".into()),
        FakeTurn::Text("Who sees the orders, the clerk or the customer?".into()),
    ]));
    let router = router(dir.path(), Some(llm.clone()));
    let cookie = ask(&router, None).await;

    let response = settle(&router, FEATURE, Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["HX-Redirect"], "/ask");
    let ended = send(&router, get_request("/ask/questions/2", Some(&cookie))).await;
    assert_eq!(ended.status(), StatusCode::OK);

    let request = llm.messages().pop().unwrap();
    assert!(
        request[0].content.contains("help settle it"),
        "{}",
        request[0].content
    );
    let roles: Vec<Role> = request.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        [Role::System, Role::User],
        "the earlier exchange is dropped"
    );
    let first = &request[1].content;
    assert!(first.contains("feature `list-orders`"), "{first}");
    assert!(
        first.contains("# List orders (`orders/list-orders`)"),
        "{first}"
    );
    assert!(
        first.contains("`README.md#Orders` states: The customer sees their orders"),
        "{first}"
    );
    assert!(
        first.contains("The section states: The clerk sees the orders"),
        "{first}"
    );

    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;
    assert!(!page.contains("Listed."), "{page}");
    assert!(page.contains("Who sees the orders"), "{page}");
    let banner = &page[page.find("class=\"settling\"").expect(&page)..];
    assert!(
        banner.contains("<a href=\"/domains/orders/list-orders\">List orders</a>"),
        "{banner}"
    );

    // STL-3: a reset ends the settling.
    send(
        &router,
        post_request("/ask/reset", "", Some(&cookie), false),
    )
    .await;
    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;
    assert!(!page.contains("class=\"settling\""), "{page}");
}

// spec: STL-10
#[tokio::test]
async fn a_settling_without_htmx_is_redirected_to_the_ask_page_at_once_with_a_new_session() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::hanging_after(0, |_| String::new()));
    let router = router(dir.path(), Some(llm));
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        send(&router, post_request("/ask/settle", FEATURE, None, false)),
    )
    .await
    .expect("answered before the model");
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/ask");
    let cookie = session_cookie(&response).expect("a session cookie");
    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;
    assert!(
        page.contains("class=\"exchange running\"") || page.contains("running"),
        "{page}"
    );
}

// spec: STL-11
#[tokio::test]
async fn a_settling_is_refused_for_an_unknown_section_or_kind_and_while_a_question_runs() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::hanging_after(1, |_| "Listed.".to_string()));
    let router = router(dir.path(), Some(llm.clone()));
    let cookie = ask(&router, None).await;
    for (form, code) in [
        ("kind=feature&id=gone", StatusCode::NOT_FOUND),
        ("kind=use_case&id=list-orders/gone", StatusCode::NOT_FOUND),
        ("kind=rule&id=list-orders", StatusCode::BAD_REQUEST),
    ] {
        let response = settle(&router, form, Some(&cookie)).await;
        assert_eq!(response.status(), code, "{form}");
    }
    assert_eq!(llm.calls(), 1);
    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;
    assert!(page.contains("Listed."), "the session is kept: {page}");

    // A question hangs in the session.
    let response = send(
        &router,
        post_request("/ask", "question=When%3F", Some(&cookie), true),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = settle(&router, FEATURE, Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;
    assert!(page.contains("Listed."), "{page}");
    assert!(!page.contains("class=\"settling\""), "{page}");

    let without = generated_repo();
    let plain = self::router(without.path(), None);
    let response = settle(&plain, FEATURE, None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
