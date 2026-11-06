//! The ask page of `retrodoc ui --ask` (`specs/web-ask.md`), asked of the router without a socket
//! over the artifacts a run leaves in `.retrodoc/cache/`, the model the fake provider of the tests.

use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::request::Builder as RequestBuilder;
use axum::http::{header, Request, Response, StatusCode};
use axum::Router;
use retrodoc_core::model::{ConfidenceScore, Feature};
use retrodoc_llm::{LlmError, LlmProvider, Role, ToolCall};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::testing::{FakeLlm, FakeTurn};
use retrodoc_pipeline::{DomainCluster, DomainMap};
use retrodoc_render::DocLinks;
use retrodoc_web::{Asking, Project};
use tower::ServiceExt;

/// A run over `orders`: the feature `list-orders`, grounded on `app/order.rb`, at a low confidence.
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
        description: "The customer sees their orders".into(),
        source_paths: vec!["app/order.rb".into()],
        entry_points: Vec::new(),
        confidence: Some(ConfidenceScore::new(0.2, None)),
    };
    retrodoc_pipeline::save_features(root, &[feature]).unwrap();
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

fn project(lang: Option<&'static str>) -> Project {
    Project {
        name: "Shop".into(),
        lang,
        ..Project::default()
    }
}

fn router_in(root: &Path, llm: Option<Arc<dyn LlmProvider>>, lang: Option<&'static str>) -> Router {
    let docs = GeneratedDocs::load(root, &[]).unwrap();
    let asking = llm.map(|llm| Asking { llm, max_rounds: 8 });
    retrodoc_web::router_with(
        project(lang),
        Some(docs),
        DocLinks::default(),
        Vec::new(),
        asking,
    )
}

fn router(root: &Path, llm: Option<Arc<dyn LlmProvider>>) -> Router {
    router_in(root, llm, Some("en"))
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
    let request = with_cookie(request, cookie, htmx);
    request.body(Body::from(form.to_string())).unwrap()
}

/// `request` to this server, with the session `cookie` and the `htmx` header when given.
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

/// The `name=value` of the session cookie a response sets, if any.
fn session_cookie(response: &Response<Body>) -> Option<String> {
    let set = response.headers().get(header::SET_COOKIE)?.to_str().ok()?;
    set.split(';').next().map(str::to_string)
}

/// Asks `question` with htmx in the session of `cookie` (a new one without), and gives the
/// session cookie and the exchange answered.
async fn ask(router: &Router, question: &str, cookie: Option<&str>) -> (String, String) {
    let form = format!("question={}", question.replace(' ', "+"));
    let response = send(router, post_request("/ask", &form, cookie, true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = session_cookie(&response)
        .or_else(|| cookie.map(str::to_string))
        .expect("a session cookie");
    (cookie, body(response).await)
}

/// The roles of the messages of the last request `llm` received.
fn last_roles(llm: &FakeLlm) -> Vec<Role> {
    llm.messages()
        .pop()
        .unwrap()
        .iter()
        .map(|m| m.role)
        .collect()
}

fn get_feature_call() -> FakeTurn {
    FakeTurn::Calls {
        text: String::new(),
        calls: vec![ToolCall {
            id: "c1".into(),
            name: "get_feature".into(),
            arguments: r#"{"id":"orders/list-orders"}"#.into(),
        }],
    }
}

/// A model that reads the feature, then answers naming it, its file, a feature it never read and
/// a raw HTML tag.
fn navigating() -> Arc<FakeLlm> {
    Arc::new(FakeLlm::scripted(vec![
        get_feature_call(),
        FakeTurn::Text(
            "The customer lists them in `orders/list-orders`, coded in `app/order.rb`; **refunds** \
             are in `orders/refund`. <script>alert(1)</script> [x](javascript:alert(1))"
                .into(),
        ),
        FakeTurn::Text("By the customer.".into()),
    ]))
}

// spec: WAK-2
#[tokio::test]
async fn without_ask_there_is_no_ask_page() {
    let dir = generated_repo();
    let router = router(dir.path(), None);

    let page = send(&router, get_request("/ask", None)).await;
    let posted = send(&router, post_request("/ask", "question=Hi", None, false)).await;

    assert_eq!(page.status(), StatusCode::NOT_FOUND);
    assert_eq!(posted.status(), StatusCode::NOT_FOUND);
    assert!(body(page).await.contains("<h1>Not found</h1>"));
}

// spec: WAK-6, WAK-15
#[tokio::test]
async fn the_ask_page_holds_the_form_and_its_indicator() {
    let dir = generated_repo();
    let router = router(dir.path(), Some(navigating()));

    let response = send(&router, get_request("/ask", None)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let page = body(response).await;
    let form = &page[page.find("<form class=\"ask\"").expect(&page)..];
    let form = &form[..form.find("</form>").unwrap()];
    assert!(form.contains("hx-post=\"/ask\""), "{form}");
    assert!(form.contains("name=\"question\""), "{form}");
    assert!(form.contains("hx-indicator=\"#ask-waiting\""), "{form}");
    assert!(form.contains("hx-disabled-elt=\"find button\""), "{form}");
    assert!(
        page.contains("id=\"ask-waiting\" class=\"htmx-indicator\""),
        "{page}"
    );
    assert!(page.contains("Reading the docs to answer"), "{page}");
}

// spec: WAK-9, WAK-10, WAK-13, WAK-14
#[tokio::test]
async fn an_answer_shows_its_markdown_its_linked_sources_and_its_warnings() {
    let dir = generated_repo();
    let router = router(dir.path(), Some(navigating()));

    let (_, exchange) = ask(&router, "How are the orders listed?", None).await;

    assert!(!exchange.contains("<html"), "{exchange}");
    assert!(
        exchange.contains("How are the orders listed?"),
        "{exchange}"
    );
    assert!(exchange.contains("<strong>refunds</strong>"), "{exchange}");
    assert!(exchange.contains("&lt;script&gt;"), "{exchange}");
    assert!(!exchange.contains("<script>"), "{exchange}");
    assert!(!exchange.contains("javascript:"), "{exchange}");
    let sources = &exchange[exchange.find("class=\"sources\"").expect(&exchange)..];
    assert!(
        sources.contains(
            "<a href=\"/domains/orders/list-orders\">feature <code>orders/list-orders</code></a>"
        ),
        "{sources}"
    );
    assert!(
        sources.contains("<a href=\"/code/app/order.rb\">file <code>app/order.rb</code></a>"),
        "{sources}"
    );
    assert!(!sources.contains("orders/refund"), "{sources}");
    let warnings = &exchange[exchange.find("class=\"warnings\"").expect(&exchange)..];
    assert!(warnings.contains("low confidence (20%)"), "{warnings}");
    let (text, listed, warned) = (
        exchange.find("refunds").unwrap(),
        exchange.find("class=\"sources\"").unwrap(),
        exchange.find("class=\"warnings\"").unwrap(),
    );
    assert!(text < listed && listed < warned, "{exchange}");
}

// spec: WAK-14
#[tokio::test]
async fn a_question_posted_without_htmx_is_redirected_to_the_page_that_shows_it() {
    let dir = generated_repo();
    let router = router(dir.path(), Some(navigating()));

    let response = send(
        &router,
        post_request("/ask", "question=How+are+the+orders+listed%3F", None, false),
    )
    .await;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/ask");
    let cookie = session_cookie(&response).unwrap();
    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;
    assert!(page.contains("How are the orders listed?"), "{page}");
    assert!(page.contains("/domains/orders/list-orders"), "{page}");
}

// spec: WAK-7, WAK-8
#[tokio::test]
async fn a_follow_up_is_answered_after_the_earlier_questions_of_its_session() {
    let dir = generated_repo();
    let llm = navigating();
    let router = router(dir.path(), Some(llm.clone()));

    let (cookie, _) = ask(&router, "How are the orders listed?", None).await;
    let (_, second) = ask(&router, "By whom?", Some(&cookie)).await;

    assert!(second.contains("By the customer."), "{second}");
    let messages = llm.messages().pop().unwrap();
    let roles: Vec<Role> = messages.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        [Role::System, Role::User, Role::Assistant, Role::User]
    );
    assert_eq!(messages[1].content, "How are the orders listed?");
    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;
    let (first, then) = (
        page.find("How are the orders listed?").unwrap(),
        page.find("By whom?").unwrap(),
    );
    assert!(first < then, "{page}");
}

// spec: WAK-18, WAK-19
#[tokio::test]
async fn each_browser_session_keeps_its_own_conversation() {
    let dir = generated_repo();
    let llm = navigating();
    let router = router(dir.path(), Some(llm.clone()));

    let response = send(&router, post_request("/ask", "question=Hi", None, true)).await;
    let set = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_string();
    let (first, _) = ask(&router, "How are the orders listed?", None).await;
    let (other, _) = ask(&router, "By whom?", Some("retrodoc_ask=0123")).await;

    let id = set
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("retrodoc_ask=")
        .unwrap();
    assert_eq!(id.len(), 32, "{set}");
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "{set}");
    for attribute in ["HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(set.contains(attribute), "{set}");
    }
    assert_ne!(first, other);
    assert_ne!(other, "retrodoc_ask=0123");
    assert_eq!(last_roles(&llm), [Role::System, Role::User]);
    let page = body(send(&router, get_request("/ask", Some(&other))).await).await;
    assert!(!page.contains("How are the orders listed?"), "{page}");
}

// spec: WAK-12
#[tokio::test]
async fn an_answer_with_no_source_or_no_text_says_so() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::sequence(&["Nothing about it.", "  "]));
    let router = router(dir.path(), Some(llm));

    let (cookie, unbacked) = ask(&router, "What about refunds?", None).await;
    let (_, blank) = ask(&router, "And returns?", Some(&cookie)).await;

    assert!(
        unbacked.contains("No source of the docs backs this answer."),
        "{unbacked}"
    );
    assert!(blank.contains("The model gave no answer."), "{blank}");
}

// spec: WAK-16
#[tokio::test]
async fn a_failed_question_shows_its_error_and_is_not_kept() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::replying(|n, _| {
        if n == 0 {
            Err(LlmError::Transport("connection refused".into()))
        } else {
            Ok("Fine.".into())
        }
    }));
    let router = router(dir.path(), Some(llm.clone()));

    let (cookie, failed) = ask(&router, "How are the orders listed?", None).await;
    ask(&router, "By whom?", Some(&cookie)).await;

    assert!(failed.contains("connection refused"), "{failed}");
    assert_eq!(last_roles(&llm), [Role::System, Role::User]);
}

// spec: WAK-17
#[tokio::test]
async fn a_blank_question_is_refused_without_a_call() {
    let dir = generated_repo();
    let llm = navigating();
    let router = router(dir.path(), Some(llm.clone()));

    let response = send(&router, post_request("/ask", "question=+++", None, true)).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(llm.calls(), 0);
}

// spec: WAK-13
#[tokio::test]
async fn the_ask_page_is_in_french_with_a_french_language() {
    let dir = generated_repo();
    let router = router_in(dir.path(), Some(navigating()), Some("fr"));

    let (_, exchange) = ask(&router, "Comment sont listées les commandes ?", None).await;
    let page = body(send(&router, get_request("/ask", None)).await).await;

    assert!(page.contains("Lecture de la doc"), "{page}");
    assert!(
        exchange.contains("fonctionnalité <code>orders/list-orders</code>"),
        "{exchange}"
    );
    assert!(exchange.contains("confiance faible (20 %)"), "{exchange}");
}

// spec: WEB-101
#[tokio::test]
async fn the_header_links_to_the_ask_page_only_with_ask() {
    let dir = generated_repo();
    let llm: Arc<dyn LlmProvider> = navigating();

    let with = body(send(&router(dir.path(), Some(llm)), get_request("/", None)).await).await;
    let without = body(send(&router(dir.path(), None), get_request("/", None)).await).await;

    let header = &with[..with.find("</header>").unwrap()];
    assert!(header.contains("href=\"/ask\""), "{header}");
    assert!(!without.contains("href=\"/ask\""), "{without}");
}

// spec: WAK-22
#[tokio::test]
async fn starting_over_empties_the_exchanges_and_the_conversation() {
    let dir = generated_repo();
    let llm = navigating();
    let router = router(dir.path(), Some(llm.clone()));
    let (cookie, _) = ask(&router, "How are the orders listed?", None).await;
    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;
    assert!(page.contains("action=\"/ask/reset\""), "{page}");

    let response = send(
        &router,
        post_request("/ask/reset", "", Some(&cookie), false),
    )
    .await;
    ask(&router, "By whom?", Some(&cookie)).await;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/ask");
    assert_eq!(last_roles(&llm), [Role::System, Role::User]);
    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;
    assert!(!page.contains("How are the orders listed?"), "{page}");
    assert!(page.contains("By whom?"), "{page}");
}

// spec: WAK-21
#[tokio::test]
async fn a_session_keeps_its_last_exchanges_for_its_page() {
    let dir = generated_repo();
    let router = router(dir.path(), Some(Arc::new(FakeLlm::answering("Yes."))));
    let (cookie, _) = ask(&router, "Question 0?", None).await;
    for n in 1..=retrodoc_web::MAX_EXCHANGES {
        ask(&router, &format!("Question {n}?"), Some(&cookie)).await;
    }

    let page = body(send(&router, get_request("/ask", Some(&cookie))).await).await;

    assert_eq!(
        page.matches("class=\"exchange\"").count(),
        retrodoc_web::MAX_EXCHANGES
    );
    assert!(!page.contains("Question 0?"), "{page}");
    assert!(page.contains("Question 1?"), "{page}");
}

// spec: WAK-25
#[tokio::test]
async fn the_page_and_the_reset_start_no_session() {
    let dir = generated_repo();
    let router = router(dir.path(), Some(navigating()));

    let page = send(&router, get_request("/ask", Some("retrodoc_ask=gone"))).await;
    let reset = send(&router, post_request("/ask/reset", "", None, false)).await;

    assert_eq!(page.status(), StatusCode::OK);
    assert_eq!(session_cookie(&page), None);
    assert_eq!(reset.status(), StatusCode::SEE_OTHER);
    assert_eq!(session_cookie(&reset), None);
    assert!(!body(page).await.contains("class=\"exchange\""));
}

// spec: WAK-26, WAK-29
#[tokio::test]
async fn the_page_of_a_session_is_served_while_its_question_runs() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::hanging_after(1, |_| "Listed by date.".to_string()));
    let router = router(dir.path(), Some(llm.clone()));
    let (cookie, _) = ask(&router, "How are the orders listed?", None).await;
    let running = tokio::spawn({
        let (router, cookie) = (router.clone(), cookie.clone());
        async move {
            send(
                &router,
                post_request("/ask", "question=By+whom%3F", Some(&cookie), true),
            )
            .await
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let page = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        send(&router, get_request("/ask", Some(&cookie))),
    )
    .await
    .expect("the page waits for the model");

    let again = send(
        &router,
        post_request("/ask", "question=When%3F", Some(&cookie), true),
    )
    .await;

    assert!(body(page).await.contains("Listed by date."));
    assert_eq!(again.status(), StatusCode::CONFLICT);
    assert_eq!(llm.calls(), 2);
    running.abort();
    let _ = running.await;
    // The question dropped with its request leaves the session free: the next one runs (and
    // hangs, as the model does from its second call) rather than being refused.
    let next = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        send(
            &router,
            post_request("/ask", "question=When%3F", Some(&cookie), true),
        ),
    )
    .await;
    assert!(next.is_err(), "refused: {:?}", next.map(|r| r.status()));
}
