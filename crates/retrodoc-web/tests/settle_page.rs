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
        assert!(
            !response.headers().contains_key("HX-Retarget")
                && !response.headers().contains_key("HX-Reswap"),
            "the section page shows it in its notice: {form}"
        );
        assert!(!body(response).await.contains('<'), "{form}");
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

fn propose(arguments: &str) -> FakeTurn {
    FakeTurn::Calls {
        text: String::new(),
        calls: vec![retrodoc_llm::ToolCall {
            id: "p1".into(),
            name: "propose_verdict".into(),
            arguments: arguments.into(),
        }],
    }
}

/// The form of the review of a section page.
fn review_form(page: &str) -> &str {
    let from = page.find("<form class=\"section-form\"").expect(page);
    &page[from..from + page[from..].find("</form>").unwrap()]
}

// spec: STL-7, STL-8, STL-9, STL-H1
#[tokio::test]
async fn a_proposal_is_shown_in_its_exchange_and_fills_the_form_saved_only_by_the_reader() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::scripted(vec![
        FakeTurn::Text("Who sees the orders, the clerk or the customer?".into()),
        propose(
            r#"{"status":"validated","comment":"The customer sees them.","evidence":"`app/order.rb` filters by customer.","description":"The customer sees their orders"}"#,
        ),
    ]));
    let router = router(dir.path(), Some(llm));
    let response = settle(&router, FEATURE, None).await;
    let cookie = session_cookie(&response).unwrap();
    send(&router, get_request("/ask/questions/1", Some(&cookie))).await;
    let response = send(
        &router,
        post_request("/ask", "question=The+customer.", Some(&cookie), true),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let exchange = body(send(&router, get_request("/ask/questions/2", Some(&cookie))).await).await;
    let proposal = &exchange[exchange.find("class=\"proposal\"").expect(&exchange)..];
    assert!(proposal.contains("Validated"), "{proposal}");
    assert!(
        proposal.contains("<p class=\"comment\">The customer sees them.</p>"),
        "{proposal}"
    );
    let evidence = &proposal[proposal
        .find("<details class=\"evidence\">")
        .expect(proposal)..];
    assert!(
        evidence[..evidence.find("</details>").unwrap()]
            .contains("<code>app/order.rb</code> filters by customer."),
        "the evidence is folded under the comment: {proposal}"
    );
    assert!(
        proposal.contains("The customer sees their orders"),
        "{proposal}"
    );
    assert!(
        proposal.contains("href=\"/domains/orders/list-orders?proposal=2#review\""),
        "{proposal}"
    );
    let reviews = dir.path().join(".retrodoc/reviews.json");
    let edits = dir.path().join(".retrodoc/edits.json");
    assert!(!reviews.exists() && !edits.exists());

    let page = body(
        send(
            &router,
            get_request("/domains/orders/list-orders?proposal=2", Some(&cookie)),
        )
        .await,
    )
    .await;
    let form = review_form(&page);
    assert!(form.contains("value=\"validated\" checked"), "{form}");
    assert!(
        form.contains(">The customer sees them.</textarea>"),
        "the comment alone, without its evidence: {form}"
    );
    assert!(!form.contains("filters by customer"), "{form}");
    assert!(
        form.contains(">The customer sees their orders</textarea>"),
        "{form}"
    );
    assert!(
        form.contains("value=\"List orders\""),
        "the name is as shown: {form}"
    );
    assert!(
        form.contains("<details class=\"text-fields\" open"),
        "{form}"
    );
    assert!(form.contains("class=\"proposed\""), "{form}");
    assert!(!reviews.exists() && !edits.exists());

    let save = "status=validated&comment=The+customer+sees+them.&name=List+orders\
                &description=The+customer+sees+their+orders\
                &shown_name=List+orders&shown_description=The+clerk+sees+the+orders";
    let response = send(
        &router,
        post_request("/domains/orders/list-orders", save, Some(&cookie), true),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(reviews.exists() && edits.exists());
    let saved = std::fs::read_to_string(&reviews).unwrap();
    assert!(saved.contains("\"status\": \"validated\""), "{saved}");
}

// spec: STL-16
#[tokio::test]
async fn a_proposal_parameter_naming_no_proposal_of_the_section_is_left_out() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::scripted(vec![
        propose(r#"{"status":"validated","comment":"Right.","evidence":"e"}"#),
        FakeTurn::Text("No proposal.".into()),
    ]));
    let router = router(dir.path(), Some(llm));
    let response = settle(&router, FEATURE, None).await;
    let cookie = session_cookie(&response).unwrap();
    send(&router, get_request("/ask/questions/1", Some(&cookie))).await;
    ask(&router, Some(&cookie)).await;
    let page = body(
        send(
            &router,
            get_request("/domains/orders/list-orders?proposal=1", Some(&cookie)),
        )
        .await,
    )
    .await;
    assert!(
        review_form(&page).contains("class=\"proposed\""),
        "the case that fills"
    );
    assert!(
        !review_form(&page).contains("<details class=\"text-fields\" open"),
        "a proposal that corrects no field unfolds none"
    );
    for (path, cookie) in [
        ("/domains/orders/list-orders?proposal=1", None),
        (
            "/domains/orders/list-orders?proposal=9",
            Some(cookie.as_str()),
        ),
        (
            "/domains/orders/list-orders?proposal=2",
            Some(cookie.as_str()),
        ),
        (
            "/domains/orders/list-orders?proposal=x",
            Some(cookie.as_str()),
        ),
        ("/domains/orders?proposal=1", Some(cookie.as_str())),
    ] {
        let page = body(send(&router, get_request(path, cookie)).await).await;
        let form = review_form(&page);
        assert!(!form.contains("class=\"proposed\""), "{path}: {form}");
        assert!(form.contains("value=\"\" checked"), "{path}: {form}");
    }
}

// spec: STL-H2, STL-4, STL-6, STL-14
#[tokio::test]
async fn a_contradicted_section_settled_from_the_code_alone_ends_on_a_proposal_at_once() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::scripted(vec![
        FakeTurn::Calls {
            text: String::new(),
            calls: vec![retrodoc_llm::ToolCall {
                id: "r1".into(),
                name: "read_source".into(),
                arguments: r#"{"path":"app/order.rb"}"#.into(),
            }],
        },
        propose(
            r#"{"status":"validated","comment":"The clerk lists them.","evidence":"`app/order.rb` lists them for the clerk."}"#,
        ),
    ]));
    let router = router(dir.path(), Some(llm.clone()));
    let cookie = session_cookie(&settle(&router, FEATURE, None).await).unwrap();
    let exchange = body(send(&router, get_request("/ask/questions/1", Some(&cookie))).await).await;
    assert!(llm.messages()[0][1]
        .content
        .contains("`README.md#Orders` states"));
    assert!(llm.tools()[0].iter().any(|tool| tool == "propose_verdict"));
    assert_eq!(llm.calls(), 2, "no question to the reader");
    let proposal = &exchange[exchange.find("class=\"proposal\"").expect(&exchange)..];
    assert!(proposal.contains("Validated"), "{proposal}");
    assert!(proposal.contains("lists them for the clerk"), "{proposal}");
}

/// The lines the server logs while `test` runs, the tasks it spawns on this thread included.
async fn logged<F: std::future::Future<Output = ()>>(test: F) -> Vec<String> {
    let (buffer, guard) = retrodoc_pipeline::testing::capture_logs();
    test.await;
    drop(guard);
    buffer.text().lines().map(str::to_string).collect()
}

// spec: STL-17
#[tokio::test]
async fn each_question_of_a_settling_logs_its_rounds_calls_and_tokens_and_the_settlings_sums() {
    let dir = generated_repo();
    let llm = Arc::new(FakeLlm::scripted(vec![
        FakeTurn::Calls {
            text: String::new(),
            calls: vec![retrodoc_llm::ToolCall {
                id: "r1".into(),
                name: "read_source".into(),
                arguments: r#"{"path":"app/order.rb"}"#.into(),
            }],
        },
        FakeTurn::Text("Who sees the orders?".into()),
        propose(r#"{"status":"wrong","comment":"The customer sees them.","evidence":"e"}"#),
        FakeTurn::Calls {
            text: String::new(),
            calls: vec![retrodoc_llm::ToolCall {
                id: "r2".into(),
                name: "read_source".into(),
                arguments: r#"{"path":"app/order.rb"}"#.into(),
            }],
        },
        FakeTurn::Fail,
        FakeTurn::Text("Orders.".into()),
    ]));
    let router = router(dir.path(), Some(llm));
    let lines = logged(async {
        let cookie = session_cookie(&settle(&router, FEATURE, None).await).unwrap();
        send(&router, get_request("/ask/questions/1", Some(&cookie))).await;
        ask(&router, Some(&cookie)).await;
        ask(&router, Some(&cookie)).await;
        send(&router, post_request("/ask/reset", "", Some(&cookie), true)).await;
        ask(&router, Some(&cookie)).await;
    })
    .await;
    let settling: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("settling of the feature `list-orders`"))
        .filter(|line| line.contains("LLM call"))
        .collect();
    assert_eq!(
        settling.len(),
        3,
        "one line per settling question: {lines:#?}"
    );
    assert!(
        settling[0].contains(
            "question 1: 1 round, 1 tool call, 2 LLM calls (2 without usage), \
             0 tokens in (0 cached), 0 out, proposed none; \
             settling: 2 LLM calls (2 without usage), 0 tokens in (0 cached), 0 out"
        ),
        "{}",
        settling[0]
    );
    assert!(
        settling[1].contains("question 2: 1 round, 1 tool call, 1 LLM call (1 without usage)")
            && settling[1].contains("proposed wrong; settling: 3 LLM calls (3 without usage)"),
        "{}",
        settling[1]
    );
    assert!(
        settling[2].contains("question 3: 1 round, 1 tool call, 1 LLM call (1 without usage)")
            && settling[2].contains("proposed none"),
        "a failed question logs its line too: {}",
        settling[2]
    );
    assert!(
        !lines
            .iter()
            .skip_while(|l| !l.contains("question 3"))
            .skip(2)
            .any(|l| l.contains("settling")),
        "a question no settling started logs no line: {lines:#?}"
    );
}

// spec: STL-19
#[tokio::test]
async fn each_question_of_a_settling_logs_its_tool_calls_in_order() {
    let dir = generated_repo();
    let read = |id: &str, name: &str, arguments: &str| retrodoc_llm::ToolCall {
        id: id.into(),
        name: name.into(),
        arguments: arguments.into(),
    };
    let llm = Arc::new(FakeLlm::scripted(vec![
        FakeTurn::Calls {
            text: String::new(),
            calls: vec![
                read("c1", "get_feature", r#"{"id":"list-orders"}"#),
                read("c2", "read_source", r#"{"path":"app/order.rb"}"#),
            ],
        },
        FakeTurn::Text("Who sees the orders?".into()),
        FakeTurn::Text("Still unsure.".into()),
    ]));
    let router = router(dir.path(), Some(llm));
    let lines = logged(async {
        let cookie = session_cookie(&settle(&router, FEATURE, None).await).unwrap();
        send(&router, get_request("/ask/questions/1", Some(&cookie))).await;
        ask(&router, Some(&cookie)).await;
    })
    .await;
    let tools: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("settling of the feature `list-orders`"))
        .filter(|line| line.contains("tool calls:"))
        .collect();
    assert_eq!(tools.len(), 2, "one line per settling question: {lines:#?}");
    assert!(
        tools[0].contains(
            "question 1, tool calls: get_feature `list-orders`, read_source `app/order.rb`"
        ),
        "{}",
        tools[0]
    );
    assert!(
        tools[1].contains("question 2, tool calls: none"),
        "{}",
        tools[1]
    );
    let usage = lines
        .iter()
        .position(|line| line.contains("question 1:"))
        .expect("usage line");
    let calls = lines
        .iter()
        .position(|line| line.contains("question 1, tool calls:"))
        .expect("tool line");
    assert!(usage < calls, "the tool line follows the usage line");
}
