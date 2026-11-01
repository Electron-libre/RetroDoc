//! The rules page, its filters and the link of every page to it, asked of the
//! router without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{BusinessRule, Feature, RuleEvidence, RuleKind, SourceRef};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use retrodoc_render::DocLinks;
use tower::ServiceExt;

fn feature(slug: &str, domain: &str, name: &str) -> Feature {
    Feature {
        back_office: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: name.into(),
        description: String::new(),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: None,
    }
}

fn rule(slug: &str, statement: &str, kind: RuleKind, evidence: Vec<RuleEvidence>) -> BusinessRule {
    BusinessRule {
        slug: slug.into(),
        statement: statement.into(),
        kind,
        entities: Vec::new(),
        use_cases: Vec::new(),
        evidence,
        confidence: None,
    }
}

/// Two domains: `billing` with the feature `pay-invoice` (a limit, cited at
/// `app/invoice.rb:40-42`, and a permission) and `shipping` with
/// `ship-order` (a validation); the rule `old-feature-1` belongs to no
/// feature of the run.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(
        root,
        &[
            feature("pay-invoice", "billing", "Pay an invoice"),
            feature("ship-order", "shipping", "Ship an order"),
        ],
    )
    .unwrap();
    retrodoc_pipeline::save_rules(
        root,
        &[
            rule(
                "pay-invoice-1",
                "A late invoice is charged",
                RuleKind::Limit,
                vec![RuleEvidence::Code {
                    source: SourceRef {
                        path: "app/invoice.rb".into(),
                        start_line: Some(40),
                        end_line: Some(42),
                    },
                }],
            ),
            rule(
                "pay-invoice-2",
                "Only a clerk refunds",
                RuleKind::Permission,
                Vec::new(),
            ),
            rule(
                "ship-order-1",
                "An order needs an address",
                RuleKind::Validation,
                vec![RuleEvidence::Test {
                    phrase: "refuses an order with no address".into(),
                    path: "spec/order_spec.rb".into(),
                }],
            ),
            rule(
                "old-feature-1",
                "A forgotten rule",
                RuleKind::Limit,
                Vec::new(),
            ),
        ],
    )
    .unwrap();
    DomainMap {
        domains: vec![
            DomainCluster {
                slug: "billing".into(),
                name: "Billing".into(),
                ..DomainCluster::default()
            },
            DomainCluster {
                slug: "shipping".into(),
                name: "Shipping".into(),
                ..DomainCluster::default()
            },
        ],
    }
    .save(root)
    .unwrap();
    dir
}

async fn get_with(root: &Path, path: &str, htmx: bool) -> (StatusCode, String) {
    let mut request = Request::get(path);
    if htmx {
        request = request.header("HX-Request", "true");
    }
    let response = retrodoc_web::router(
        GeneratedDocs::load(root, &[]),
        DocLinks::default(),
        Vec::new(),
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

async fn rules(root: &Path, path: &str) -> String {
    let (status, page) = get_with(root, path, false).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

/// The list of the rules of the page, without the form.
fn list(page: &str) -> &str {
    let start = page.find("id=\"rule-list\"").expect(page);
    &page[start..]
}

const ALL: [&str; 4] = [
    "A late invoice is charged",
    "Only a clerk refunds",
    "An order needs an address",
    "A forgotten rule",
];

fn shown(page: &str) -> Vec<&'static str> {
    let list = list(page);
    ALL.into_iter().filter(|s| list.contains(s)).collect()
}

// spec: WEB-52
#[tokio::test]
async fn every_page_links_to_the_rules_page() {
    let repo = generated_repo();
    for path in ["/", "/files", "/rules", "/unknown"] {
        let (_, page) = get_with(repo.path(), path, false).await;
        let header = &page[page.find("<header").unwrap()..page.find("</header>").unwrap()];
        assert!(header.contains("href=\"/rules\""), "{path}: {header}");
    }
}

// spec: WEB-53
#[tokio::test]
async fn the_rules_page_lists_every_rule_in_the_order_of_the_run() {
    let repo = generated_repo();
    let page = rules(repo.path(), "/rules").await;
    let list = list(&page);
    let positions: Vec<usize> = ALL.iter().map(|s| list.find(s).expect(s)).collect();
    assert!(positions.is_sorted(), "{positions:?} in {list}");
    assert!(list.contains("(Limit, "), "{list}");
    assert!(list.contains("(Validation, "), "{list}");
    assert!(
        list.contains("<a href=\"/domains/billing/pay-invoice#rule-pay-invoice-1\">"),
        "{list}"
    );
    assert!(list.contains("Pay an invoice"), "{list}");
    assert!(list.contains("Billing"), "{list}");
    assert!(
        list.contains("<a href=\"/code/app/invoice.rb?lines=40-42#L40\">"),
        "{list}"
    );
    assert!(list.contains("refuses an order with no address"), "{list}");
}

// spec: WEB-53
#[tokio::test]
async fn a_rule_of_a_feature_not_in_the_run_is_listed_without_a_link() {
    let repo = generated_repo();
    let page = rules(repo.path(), "/rules").await;
    let list = list(&page);
    let forgotten = &list[list.find("A forgotten rule").unwrap()..];
    let item = &forgotten[..forgotten.find("</li>").unwrap()];
    assert!(!item.contains("href="), "{item}");
    let page = rules(repo.path(), "/rules?domain=billing").await;
    assert!(!shown(&page).contains(&"A forgotten rule"));
}

// spec: WEB-54
#[tokio::test]
async fn the_filters_keep_the_rules_of_a_kind_a_domain_and_a_feature() {
    let repo = generated_repo();
    let root = repo.path();
    let page = rules(root, "/rules?kind=limit").await;
    assert_eq!(
        shown(&page),
        ["A late invoice is charged", "A forgotten rule"]
    );
    let page = rules(root, "/rules?domain=shipping").await;
    assert_eq!(shown(&page), ["An order needs an address"]);
    let page = rules(root, "/rules?feature=pay-invoice").await;
    assert_eq!(
        shown(&page),
        ["A late invoice is charged", "Only a clerk refunds"]
    );
    let page = rules(root, "/rules?kind=permission&domain=billing").await;
    assert_eq!(shown(&page), ["Only a clerk refunds"]);
    let page = rules(root, "/rules?kind=state_transition").await;
    assert_eq!(shown(&page), [] as [&str; 0]);
    assert!(list(&page).contains("No rule matches"), "{page}");
}

// spec: WEB-54
#[tokio::test]
async fn an_empty_filter_filters_nothing_and_an_unknown_one_keeps_no_rule() {
    let repo = generated_repo();
    let page = rules(repo.path(), "/rules?kind=&domain=&feature=").await;
    assert_eq!(shown(&page), ALL);
    for query in ["kind=rate", "domain=nowhere", "feature=nothing"] {
        let page = rules(repo.path(), &format!("/rules?{query}")).await;
        assert_eq!(shown(&page), [] as [&str; 0], "{query}: {page}");
    }
}

// spec: WEB-55
#[tokio::test]
async fn the_form_offers_the_kinds_domains_and_features_with_the_query_selected() {
    let repo = generated_repo();
    let page = rules(repo.path(), "/rules?kind=permission&feature=ship-order").await;
    let form = &page[page.find("<form class=\"filters\"").expect(&page)..];
    let form = &form[..form.find("</form>").unwrap()];
    assert!(form.contains("action=\"/rules\""), "{form}");
    assert!(form.contains("hx-get=\"/rules\""), "{form}");
    for value in [
        "validation",
        "permission",
        "state_transition",
        "limit",
        "computation",
        "billing",
        "shipping",
        "pay-invoice",
        "ship-order",
    ] {
        assert!(
            form.contains(&format!("value=\"{value}\"")),
            "{value}: {form}"
        );
    }
    assert!(form.contains("value=\"permission\" selected"), "{form}");
    assert!(form.contains("value=\"ship-order\" selected"), "{form}");
    assert!(!form.contains("value=\"limit\" selected"), "{form}");
}

// spec: WEB-56
#[tokio::test]
async fn an_htmx_request_gets_the_list_alone() {
    let repo = generated_repo();
    let (status, fragment) = get_with(repo.path(), "/rules?kind=validation", true).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!fragment.contains("<html"), "{fragment}");
    assert!(!fragment.contains("<form"), "{fragment}");
    assert!(fragment.contains("id=\"rule-list\""), "{fragment}");
    assert_eq!(shown(&fragment), ["An order needs an address"]);
}

// spec: WEB-53
#[tokio::test]
async fn with_no_run_the_rules_page_says_to_run_generate() {
    let dir = tempfile::tempdir().unwrap();
    let (status, page) = get_with(dir.path(), "/rules", false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("retrodoc generate"), "{page}");
}
