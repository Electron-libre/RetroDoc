//! The search page and the search box of every page, asked of the router
//! without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{BusinessRule, ConfidenceScore, Feature, RuleKind, UseCase};
use retrodoc_mcp::search::SEARCH_LIMIT;
use retrodoc_mcp::{ExistingDoc, GeneratedDocs};
use retrodoc_pipeline::glossary::ModelFile;
use retrodoc_pipeline::{DomainCluster, DomainMap, Entity, Glossary};
use retrodoc_render::DocLinks;
use tower::ServiceExt;

fn feature(slug: &str, name: &str) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: "billing".into(),
        name: name.into(),
        description: String::new(),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: Some(ConfidenceScore::new(0.8, None)),
    }
}

fn collected() -> Vec<ExistingDoc> {
    vec![ExistingDoc {
        path: PathBuf::from("docs/invoice fees.md"),
        content: "# Invoice fees\nA late invoice costs more.".into(),
    }]
}

/// A run where `invoice` names one domain, feature, use case, rule, concept
/// and collected doc, and `ledger` names 12 features.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut features = vec![
        feature("pay-invoice", "Pay an invoice"),
        feature("cancel-plan", "Cancel a plan"),
    ];
    features.extend((1..=12).map(|n| feature(&format!("ledger-{n}"), &format!("Ledger {n}"))));
    retrodoc_pipeline::save_features(root, &features).unwrap();
    retrodoc_pipeline::save_use_cases(
        root,
        &[UseCase {
            slug: "by-card".into(),
            feature_slug: "pay-invoice".into(),
            name: "Pay the invoice by card".into(),
            description: String::new(),
            steps: Vec::new(),
            entry_points: Vec::new(),
            primary_actor: None,
            narrative: None,
            business_language: None,
            diagram_mermaid: None,
            confidence: Some(ConfidenceScore::new(0.3, None)),
            narrative_check: None,
        }],
    )
    .unwrap();
    retrodoc_pipeline::save_rules(
        root,
        &[BusinessRule {
            slug: "pay-invoice-1".into(),
            statement: "An invoice is paid within 30 days".into(),
            kind: RuleKind::Limit,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: Vec::new(),
            confidence: None,
        }],
    )
    .unwrap();
    Glossary {
        models: [(
            PathBuf::from("app/invoice_line.rb"),
            ModelFile {
                content_hash: String::new(),
                entities: vec![Entity {
                    name: "Invoice Line".into(),
                    description: "One row of an invoice".into(),
                    attributes: Vec::new(),
                    associations: Vec::new(),
                }],
            },
        )]
        .into(),
        tests: Vec::new(),
    }
    .save(root)
    .unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Invoice desk".into(),
            description: String::new(),
            ..DomainCluster::default()
        }],
    }
    .save(root)
    .unwrap();
    dir
}

fn load(root: &Path) -> Option<GeneratedDocs> {
    GeneratedDocs::load(root, &collected())
}

async fn get_with(root: &Path, path: &str, htmx: bool) -> (StatusCode, String) {
    let mut request = Request::get(path);
    if htmx {
        request = request.header("HX-Request", "true");
    }
    let response = retrodoc_web::router(
        retrodoc_web::Project::default(),
        load(root),
        DocLinks::default(),
        collected(),
    )
    .oneshot(request.body(Body::empty()).unwrap())
    .await
    .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn search(root: &Path, query: &str) -> String {
    let (status, page) = get_with(root, &format!("/search?q={query}"), false).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

/// The results, without the layout.
fn results(page: &str) -> &str {
    let start = page.find("<section class=\"results\"").expect(page);
    &page[start..start + page[start..].find("</section>").unwrap()]
}

/// The hit whose link is `href`, up to the end of its line of the list.
fn hit<'a>(results: &'a str, href: &str) -> &'a str {
    let start = results
        .find(&format!("href=\"{href}\""))
        .unwrap_or_else(|| panic!("no hit {href} in {results}"));
    let rest = &results[start..];
    &rest[..rest.find("</li>").unwrap()]
}

// spec: WEB-39
#[tokio::test]
async fn every_page_shows_a_search_box_sending_to_the_search_page() {
    let repo = generated_repo();
    for path in ["/", "/glossary", "/unknown", "/search?q=invoice"] {
        let (_, page) = get_with(repo.path(), path, false).await;
        let form = &page[page.find("<form class=\"search\"").expect(&page)..];
        let form = &form[..form.find("</form>").unwrap()];
        assert!(form.contains("action=\"/search\""), "{form}");
        assert!(form.contains("method=\"get\""), "{form}");
        assert!(form.contains("name=\"q\""), "{form}");
        let input = &form[form.find("<input").unwrap()..];
        let input = &input[..input.find('>').unwrap()];
        for attribute in [
            "hx-get=\"/search\"",
            "hx-trigger=\"input changed delay:300ms, search\"",
            "hx-sync=\"this:replace\"",
            "hx-target=\"#search-panel\"",
        ] {
            assert!(input.contains(attribute), "{attribute} in {input}");
        }
        assert!(
            form.contains("<div id=\"search-panel\" class=\"search-panel\""),
            "{form}"
        );
    }
}

// spec: WEB-40
#[tokio::test]
async fn the_hits_are_grouped_by_kind_in_a_fixed_order() {
    let repo = generated_repo();
    let page = search(repo.path(), "invoice").await;
    let found = results(&page);
    let positions: Vec<usize> = [
        "/domains/billing\"",
        "/domains/billing/pay-invoice\"",
        "/domains/billing/pay-invoice/by-card\"",
        "/domains/billing/pay-invoice#rule-pay-invoice-1\"",
        "/glossary#invoice-line\"",
        "/docs/docs/invoice%20fees.md\"",
    ]
    .iter()
    .map(|href| {
        found
            .find(href)
            .unwrap_or_else(|| panic!("{href} in {found}"))
    })
    .collect();
    assert!(positions.is_sorted(), "{positions:?} in {found}");
    for kind in [
        "Domains",
        "Features",
        "Use cases",
        "Business rules",
        "Glossary",
        "Existing docs",
    ] {
        assert!(
            found.contains(&format!("<h2>{kind}</h2>")),
            "{kind} in {found}"
        );
    }
}

// spec: WEB-40
#[tokio::test]
async fn the_hits_are_those_of_the_index_up_to_its_limit() {
    let repo = generated_repo();
    let page = search(repo.path(), "ledger").await;
    let index = load(repo.path()).unwrap();
    let expected: Vec<String> = index
        .index()
        .search("ledger", SEARCH_LIMIT)
        .hits
        .iter()
        .map(|h| format!(">{}</a>", h.entry.title))
        .collect();
    assert_eq!(expected.len(), 10);
    let found = results(&page);
    assert_eq!(found.matches("<li").count(), 10, "{found}");
    let mut from = 0;
    for title in &expected {
        let at = found[from..].find(title.as_str()).expect(title);
        from += at + title.len();
    }
}

// spec: WEB-41
#[tokio::test]
async fn each_hit_shows_its_title_and_confidence_with_a_link() {
    let repo = generated_repo();
    let page = search(repo.path(), "invoice").await;
    let found = results(&page);
    let feature = hit(found, "/domains/billing/pay-invoice");
    assert!(feature.contains(">Pay an invoice</a>"), "{feature}");
    assert!(feature.contains("80%"), "{feature}");
    let use_case = hit(found, "/domains/billing/pay-invoice/by-card");
    assert!(use_case.contains("class=\"badge low\""), "{use_case}");
    let concept = hit(found, "/glossary#invoice-line");
    assert!(concept.contains(">Invoice Line</a>"), "{concept}");
    assert!(!concept.contains("badge"), "{concept}");
    assert!(hit(found, "/docs/docs/invoice%20fees.md").contains(">Invoice fees</a>"));
}

// spec: WEB-42
#[tokio::test]
async fn no_hit_says_not_documented_with_the_words_no_entry_has() {
    let repo = generated_repo();
    let page = search(repo.path(), "invoice+zzqx").await;
    let found = results(&page);
    let left_out = load(repo.path())
        .unwrap()
        .index()
        .search("invoice zzqx", SEARCH_LIMIT)
        .left_out;
    assert!(left_out > 0);
    assert!(found.contains("Not documented"), "{found}");
    assert!(found.contains("<code>zzqx</code>"), "{found}");
    assert!(
        found.contains(&format!("{left_out} entries share only part")),
        "{found}"
    );
    assert!(!found.contains("<li"), "{found}");
}

// spec: WEB-42
#[tokio::test]
async fn the_words_of_the_query_are_shown_as_text() {
    let repo = generated_repo();
    let page = search(repo.path(), "%3Cscript%3Ezzqx%3C%2Fscript%3E").await;
    assert!(!page.contains("<script>zzqx"), "{page}");
    assert!(results(&page).contains("Not documented"), "{page}");
}

// spec: WEB-40
#[tokio::test]
async fn an_empty_query_shows_no_result() {
    let repo = generated_repo();
    for path in ["/search", "/search?q=", "/search?q=+"] {
        let (status, page) = get_with(repo.path(), path, false).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!page.contains("Not documented"), "{path}: {page}");
        assert!(
            !page.contains("<section class=\"results\""),
            "{path}: {page}"
        );
    }
}

// spec: WEB-43
#[tokio::test]
async fn an_htmx_request_gets_the_panel_alone() {
    let repo = generated_repo();
    let (status, fragment) = get_with(repo.path(), "/search?q=invoice", true).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        fragment.trim_start().starts_with("<a class=\"hit\""),
        "{fragment}"
    );
    assert!(!fragment.contains("<html"), "{fragment}");
    assert!(!fragment.contains("<nav class=\"tree\""), "{fragment}");
    let (_, page) = get_with(repo.path(), "/search?q=invoice", false).await;
    assert!(page.contains("<html"), "{page}");
}

// spec: WEB-43
#[tokio::test]
async fn an_htmx_query_under_two_characters_gets_an_empty_answer() {
    let repo = generated_repo();
    for query in ["", "i", "+i+", "%C3%A9"] {
        let (status, fragment) = get_with(repo.path(), &format!("/search?q={query}"), true).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        assert_eq!(fragment, "", "{query}");
    }
    let (_, page) = get_with(repo.path(), "/search?q=i", false).await;
    assert!(page.contains("<section class=\"results\""), "{page}");
}

// spec: WEB-109
#[tokio::test]
async fn the_panel_lists_each_hit_with_its_kind_then_the_search_page() {
    let repo = generated_repo();
    let (_, panel) = get_with(repo.path(), "/search?q=invoice", true).await;
    let rows: Vec<&str> = panel.split("<a class=\"hit").skip(1).collect();
    let expected = [
        ("/domains/billing\"", "Domain"),
        ("/domains/billing/pay-invoice\"", "Feature"),
        ("/domains/billing/pay-invoice/by-card\"", "Use case"),
        (
            "/domains/billing/pay-invoice#rule-pay-invoice-1\"",
            "Business rule",
        ),
        ("/glossary#invoice-line\"", "Concept"),
        ("/docs/docs/invoice%20fees.md\"", "Existing doc"),
    ];
    assert_eq!(rows.len(), expected.len() + 1, "{panel}");
    for (row, (href, kind)) in rows.iter().zip(expected) {
        assert!(row.contains(href), "{href} in {row}");
        assert!(
            row.contains(&format!("<span class=\"kind\">{kind}</span>")),
            "{kind} in {row}"
        );
    }
    assert!(
        rows[1].contains("<span class=\"title\">Pay an invoice</span>"),
        "{}",
        rows[1]
    );
    assert!(rows[1].contains("80%"), "{}", rows[1]);
    assert!(!rows[4].contains("badge"), "{}", rows[4]);
    let all = rows.last().unwrap();
    assert!(all.contains(" href=\"/search?q=invoice\""), "{all}");
}

// spec: WEB-109
#[tokio::test]
async fn the_panel_shows_the_hits_of_the_search_page() {
    let repo = generated_repo();
    let (_, panel) = get_with(repo.path(), "/search?q=ledger", true).await;
    assert_eq!(
        panel.matches("<a class=\"hit\"").count(),
        SEARCH_LIMIT,
        "{panel}"
    );
    assert!(
        panel.contains(
            "<a class=\"hit all\" role=\"option\" id=\"search-hit-all\" href=\"/search?q=ledger\""
        ),
        "{panel}"
    );
    let (_, panel) = get_with(repo.path(), "/search?q=pay+invoice", true).await;
    assert!(
        panel.contains("<a class=\"hit all\" role=\"option\" id=\"search-hit-all\" href=\"/search?q=pay%20invoice\""),
        "{panel}"
    );
}

// spec: WEB-109
#[tokio::test]
async fn the_panel_with_no_hit_says_not_documented() {
    let repo = generated_repo();
    let (_, panel) = get_with(repo.path(), "/search?q=zzqx", true).await;
    assert!(panel.contains("Not documented"), "{panel}");
    assert!(!panel.contains("<a class=\"hit\""), "{panel}");
    assert!(
        panel.contains(
            "<a class=\"hit all\" role=\"option\" id=\"search-hit-all\" href=\"/search?q=zzqx\""
        ),
        "{panel}"
    );
}

// spec: WEB-43
#[tokio::test]
async fn an_htmx_history_restore_gets_the_full_search_page() {
    let repo = generated_repo();
    let request = Request::get("/search?q=invoice")
        .header("HX-Request", "true")
        .header("HX-History-Restore-Request", "true");
    let response = retrodoc_web::router(
        retrodoc_web::Project::default(),
        load(repo.path()),
        DocLinks::default(),
        collected(),
    )
    .oneshot(request.body(Body::empty()).unwrap())
    .await
    .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let page = String::from_utf8(body.to_vec()).unwrap();
    assert!(page.contains("<html"), "{page}");
}

// spec: WEB-41
#[tokio::test]
async fn a_rule_whose_feature_is_not_in_the_run_has_no_link() {
    let repo = generated_repo();
    retrodoc_pipeline::save_rules(
        repo.path(),
        &[BusinessRule {
            slug: "gone-1".into(),
            statement: "A refund is approved".into(),
            kind: RuleKind::Permission,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: Vec::new(),
            confidence: None,
        }],
    )
    .unwrap();
    let page = search(repo.path(), "refund").await;
    let found = results(&page);
    assert!(found.contains("<li>A refund is approved</li>"), "{found}");
}

// spec: WEB-43
#[tokio::test]
async fn the_search_page_shows_its_results_in_its_content() {
    let repo = generated_repo();
    let page = search(repo.path(), "invoice").await;
    let main = &page[page.find("<main>").expect(&page)..];
    assert!(main.contains("<section class=\"results\""), "{page}");
    assert_eq!(
        page.matches("<section class=\"results\"").count(),
        1,
        "{page}"
    );
    assert!(
        page.contains("<div id=\"search-panel\" class=\"search-panel\" role=\"listbox\" aria-label=\"Search the docs\"></div>"),
        "{page}"
    );
}

// spec: WEB-43
#[tokio::test]
async fn an_htmx_search_with_no_run_gets_a_fragment() {
    let response = retrodoc_web::router(
        retrodoc_web::Project::default(),
        None,
        DocLinks::default(),
        collected(),
    )
    .oneshot(
        Request::get("/search?q=invoice")
            .header("HX-Request", "true")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        response.headers()["vary"],
        "HX-Request, HX-History-Restore-Request"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let fragment = String::from_utf8(body.to_vec()).unwrap();
    assert!(fragment.starts_with("<p class=\"empty\""), "{fragment}");
    assert!(
        fragment.contains("<code>retrodoc generate</code>"),
        "{fragment}"
    );
}

// spec: WEB-110
#[tokio::test]
async fn the_answer_with_no_hit_is_an_option_the_arrows_skip() {
    let repo = generated_repo();
    let (_, panel) = get_with(repo.path(), "/search?q=zzqx", true).await;
    assert!(
        panel.contains(
            "<p class=\"empty\" role=\"option\" aria-disabled=\"true\" id=\"search-hit-none\">"
        ),
        "{panel}"
    );
    let (_, script) = get_with(repo.path(), "/assets/search.js", false).await;
    assert!(
        script.contains("[role=option]:not([aria-disabled=true])"),
        "{script}"
    );
}

// spec: WEB-110
#[tokio::test]
async fn the_search_box_is_a_combobox_over_the_panel() {
    let repo = generated_repo();
    let (_, page) = get_with(repo.path(), "/", false).await;
    let input = &page[page.find("<input type=\"search\"").expect(&page)..];
    let input = &input[..input.find('>').unwrap()];
    for attribute in [
        "role=\"combobox\"",
        "aria-autocomplete=\"list\"",
        "aria-controls=\"search-panel\"",
        "aria-expanded=\"false\"",
    ] {
        assert!(input.contains(attribute), "{attribute} in {input}");
    }
    assert!(
        page.contains("<div id=\"search-panel\" class=\"search-panel\" role=\"listbox\""),
        "{page}"
    );
    let (_, panel) = get_with(repo.path(), "/search?q=invoice", true).await;
    let options: Vec<&str> = panel
        .split("role=\"option\" id=\"")
        .skip(1)
        .map(|rest| &rest[..rest.find('"').unwrap()])
        .collect();
    assert_eq!(options.len(), 7, "{panel}");
    let mut unique = options.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), options.len(), "{options:?}");
    assert_eq!(
        panel.matches("class=\"hit").count(),
        options.len(),
        "{panel}"
    );
}

// spec: WEB-111, WEB-112 (internal: the keys run in the browser, the test reads what it is served)
#[tokio::test]
async fn the_keys_of_the_search_box_are_in_its_script() {
    let repo = generated_repo();
    let (_, page) = get_with(repo.path(), "/glossary", false).await;
    assert!(
        page.contains("<script src=\"/assets/search.js\" defer></script>"),
        "{page}"
    );
    let (status, script) = get_with(repo.path(), "/assets/search.js", false).await;
    assert_eq!(status, StatusCode::OK);
    for part in [
        "\"/\"",
        "isContentEditable",
        "\"TEXTAREA\"",
        "\"SELECT\"",
        ".select()",
        "\"ArrowDown\"",
        "\"ArrowUp\"",
        "\"Enter\"",
        "\"Escape\"",
        "aria-activedescendant",
        "aria-selected",
        "aria-expanded",
        "htmx:afterSwap",
        "preventDefault",
    ] {
        assert!(script.contains(part), "{part} in search.js");
    }
}
