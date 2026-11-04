//! The debt page, its filters and the links of every page to it, asked of the
//! router without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{Actor, ActorKind, ConfidenceScore, Feature, SourceRef, Step, UseCase};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::{DomainCluster, DomainMap, SourceHashes};
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use tower::ServiceExt;

fn score(value: f32, why: &str) -> ConfidenceScore {
    ConfidenceScore::new(value, why.to_string())
}

fn feature(slug: &str, domain: &str, name: &str) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: name.into(),
        description: String::new(),
        source_paths: vec![format!("app/{slug}.rb")],
        entry_points: Vec::new(),
        confidence: Some(score(0.9, "fine")),
    }
}

fn use_case(slug: &str, feature: &str, name: &str, confidence: Option<f32>) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: slug.into(),
        feature_slug: feature.into(),
        name: name.into(),
        description: String::new(),
        steps: vec![Step {
            order: 1,
            description: "Acts".into(),
            actor: Actor {
                name: "Clerk".into(),
                kind: ActorKind::Human,
            },
            action: String::new(),
            source_refs: vec![SourceRef {
                path: format!("app/{feature}.rb"),
                start_line: Some(1),
                end_line: None,
            }],
        }],
        diagram_mermaid: None,
        confidence: confidence.map(|c| score(c, "few cited lines")),
        narrative_check: None,
    }
}

/// `billing` (feature `pay-invoice`: `Refund <script>` low at 30%, `Pay by card` not scored)
/// and `shipping` (feature `ship-order`, whose file changed since the run: `Ship` stale).
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
    retrodoc_pipeline::save_use_cases(
        root,
        &[
            use_case("refund", "pay-invoice", "Refund <script>", Some(0.3)),
            use_case("pay-by-card", "pay-invoice", "Pay by card", None),
            use_case("ship", "ship-order", "Ship", Some(0.9)),
        ],
    )
    .unwrap();
    DomainMap {
        domains: ["billing", "shipping"]
            .map(|slug| DomainCluster {
                slug: slug.into(),
                name: format!("{}{}", slug[..1].to_uppercase(), &slug[1..]),
                ..DomainCluster::default()
            })
            .to_vec(),
    }
    .save(root)
    .unwrap();
    std::fs::create_dir_all(root.join("app")).unwrap();
    let mut hashes = SourceHashes::default();
    for name in ["app/pay-invoice.rb", "app/ship-order.rb"] {
        std::fs::write(root.join(name), name).unwrap();
        hashes.put(name, &hash_content(name));
    }
    hashes.save(root).unwrap();
    std::fs::write(root.join("app/ship-order.rb"), "edited").unwrap();
    dir
}

async fn get_in(
    lang: Option<&'static str>,
    root: &Path,
    path: &str,
    headers: &[&str],
) -> (StatusCode, String) {
    let mut request = Request::get(path);
    for name in headers {
        request = request.header(*name, "true");
    }
    let project = Project {
        lang,
        ..Project::default()
    };
    let response = retrodoc_web::router(
        project,
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

async fn debt(root: &Path, path: &str) -> String {
    let (status, page) = get_in(None, root, path, &[]).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

/// The list of the items, without the form.
fn list(page: &str) -> &str {
    let start = page.find("id=\"debt-list\"").expect(page);
    &page[start..]
}

/// The items of the list, one string per `<li class="debt-item">`.
fn items(page: &str) -> Vec<&str> {
    list(page)
        .split("<li class=\"debt-item\"")
        .skip(1)
        .collect()
}

fn shown(page: &str) -> Vec<&'static str> {
    let list = list(page);
    [
        "Refund &#60;script&#62;",
        "Pay by card",
        ">Ship<",
        ">Ship an order<",
    ]
    .into_iter()
    .filter(|s| list.contains(s))
    .collect()
}

// spec: DBT-1, DBT-2, DBT-3, DBT-7, DBT-8
#[tokio::test]
async fn the_debt_page_lists_each_section_that_needs_a_human() {
    let repo = generated_repo();
    let page = debt(repo.path(), "/debt").await;
    let items = items(&page);

    let low = items[0];
    assert!(low.contains("Low confidence"), "{low}");
    assert!(low.contains("Billing"), "{low}");
    assert!(
        low.contains("<a href=\"/domains/billing/pay-invoice/refund\">"),
        "{low}"
    );
    assert!(low.contains("Refund &#60;script&#62;"), "{low}");
    assert!(low.contains("Pay an invoice"), "{low}");
    assert!(
        low.contains("30%") && low.contains("few cited lines"),
        "{low}"
    );
    // 30% is under the confidence that sets a use case apart too.
    let kinds: Vec<&str> = items
        .iter()
        .map(|item| {
            between(item, "class=\"badge kind\">", "</span>")
                .trim_start_matches("class=\"badge kind\">")
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "Low confidence",
            "Not scored",
            "Set apart",
            "Stale",
            "Stale"
        ],
        "{page}"
    );
    assert!(items[1].contains("/domains/billing/pay-invoice/pay-by-card"));
    assert!(items[2].contains("confidence"), "{}", items[2]);
    assert!(
        items[3].contains("<a href=\"/domains/shipping/ship-order\">Ship an order</a>"),
        "{}",
        items[3]
    );
    assert!(items[3].contains("app/ship-order.rb"), "{}", items[3]);
    assert!(items[4].contains("/domains/shipping/ship-order/ship"));
}

// spec: DBT-3
#[tokio::test]
async fn a_run_with_no_score_says_so_rather_than_listing_every_use_case() {
    let repo = generated_repo();
    retrodoc_pipeline::save_use_cases(
        repo.path(),
        &[use_case("refund", "pay-invoice", "Refund", None)],
    )
    .unwrap();
    let page = debt(repo.path(), "/debt").await;
    assert!(list(&page).contains("no confidence score"), "{page}");
    assert!(!list(&page).contains("Not scored"), "{page}");
}

// spec: DBT-3, DBT-11
#[tokio::test]
async fn a_run_with_no_score_and_nothing_else_does_not_say_the_docs_need_no_human() {
    let clean = tempfile::tempdir().unwrap();
    let mut unscored = feature("pay-invoice", "billing", "Pay an invoice");
    unscored.confidence = None;
    retrodoc_pipeline::save_features(clean.path(), &[unscored]).unwrap();
    retrodoc_pipeline::save_use_cases(
        clean.path(),
        &[use_case("refund", "pay-invoice", "Refund", None)],
    )
    .unwrap();
    let page = debt(clean.path(), "/debt").await;
    assert!(list(&page).contains("no confidence score"), "{page}");
    assert!(!list(&page).contains("needs a human"), "{page}");
}

// spec: DBT-9
#[tokio::test]
async fn the_debt_page_keeps_the_items_of_the_kind_and_the_domain_asked() {
    let repo = generated_repo();
    for (query, expected) in [
        (
            "",
            &[
                "Refund &#60;script&#62;",
                "Pay by card",
                ">Ship<",
                ">Ship an order<",
            ][..],
        ),
        (
            "?kind=",
            &[
                "Refund &#60;script&#62;",
                "Pay by card",
                ">Ship<",
                ">Ship an order<",
            ][..],
        ),
        ("?kind=not_scored", &["Pay by card"][..]),
        ("?domain=shipping", &[">Ship<", ">Ship an order<"][..]),
        ("?kind=stale&domain=billing", &[][..]),
        (
            "?kind=low_confidence&domain=billing",
            &["Refund &#60;script&#62;"][..],
        ),
        ("?kind=nonsense", &[][..]),
    ] {
        let page = debt(repo.path(), &format!("/debt{query}")).await;
        assert_eq!(shown(&page), expected, "{query}");
    }
}

// spec: DBT-10
#[tokio::test]
async fn the_debt_form_offers_the_kinds_and_the_domains_with_the_asked_ones_selected() {
    let repo = generated_repo();
    let page = debt(repo.path(), "/debt?kind=stale&domain=shipping").await;
    let form = between(&page, "<form class=\"filters\"", "</form>");
    assert!(form.contains("hx-get=\"/debt\""), "{form}");
    for kind in [
        "low_confidence",
        "not_scored",
        "unbacked_narrative",
        "technical_language",
        "set_apart",
    ] {
        assert!(
            form.contains(&format!("<option value=\"{kind}\">")),
            "{kind}: {form}"
        );
    }
    assert!(form.contains("<option value=\"stale\" selected>"), "{form}");
    assert!(
        form.contains("<option value=\"billing\">Billing</option>"),
        "{form}"
    );
    assert!(
        form.contains("<option value=\"shipping\" selected>Shipping</option>"),
        "{form}"
    );
}

// spec: DBT-11
#[tokio::test]
async fn the_debt_page_says_when_nothing_needs_a_human_or_nothing_matches() {
    let repo = generated_repo();
    let page = debt(repo.path(), "/debt?kind=unbacked_narrative").await;
    assert!(list(&page).contains("No item matches."), "{page}");

    let clean = tempfile::tempdir().unwrap();
    let mut fine = feature("pay-invoice", "billing", "Pay an invoice");
    fine.source_paths.clear();
    retrodoc_pipeline::save_features(clean.path(), &[fine]).unwrap();
    let page = debt(clean.path(), "/debt").await;
    assert!(
        list(&page).contains("No section of the docs needs a human."),
        "{page}"
    );
}

// spec: DBT-12
#[tokio::test]
async fn an_htmx_debt_request_gets_the_list_alone() {
    let repo = generated_repo();
    let (_, fragment) = get_in(None, repo.path(), "/debt?kind=not_scored", &["HX-Request"]).await;
    assert!(
        fragment
            .trim_start()
            .starts_with("<section id=\"debt-list\""),
        "{fragment}"
    );
    assert!(!fragment.contains("<header"), "{fragment}");
    assert!(fragment.contains("Pay by card"), "{fragment}");
    let (_, restored) = get_in(
        None,
        repo.path(),
        "/debt",
        &["HX-Request", "HX-History-Restore-Request"],
    )
    .await;
    assert!(restored.contains("<header"), "{restored}");
}

// spec: DBT-13
#[tokio::test]
async fn every_page_links_to_the_debt_page_under_audit_and_the_home_page_counts_its_items() {
    let repo = generated_repo();
    let empty = tempfile::tempdir().unwrap();
    for (root, path) in [
        (repo.path(), "/"),
        (repo.path(), "/rules"),
        (repo.path(), "/debt"),
        (repo.path(), "/unknown"),
        (empty.path(), "/"),
    ] {
        let (_, page) = get_in(None, root, path, &[]).await;
        let header = between(&page, "<header", "</header>");
        let nav = between(header, "aria-label=\"Audit\"", "</nav>");
        let link = between(nav, "href=\"/debt\"", "</a>");
        assert!(link.contains(" title=\""), "{path}: {nav}");
    }
    let home = debt(repo.path(), "/").await;
    let start = between(&home, "id=\"start-here\"", "</section>");
    let item = between(start, "href=\"/debt\"", "</li>");
    assert!(item.contains("5 items"), "{item}");
}

// spec: DBT-13
#[tokio::test]
async fn the_count_of_the_home_page_follows_a_file_changed_after_the_start() {
    let repo = generated_repo();
    let router = retrodoc_web::router(
        Project::default(),
        GeneratedDocs::load(repo.path(), &[]),
        DocLinks::default(),
        Vec::new(),
    );
    let count = |router: axum::Router| async move {
        let response = router
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let page = String::from_utf8(body.to_vec()).unwrap();
        let start = between(&page, "id=\"start-here\"", "</section>").to_string();
        between(&start, "href=\"/debt\"", "</li>").to_string()
    };
    assert!(count(router.clone()).await.contains("5 items"));
    // The feature `pay-invoice` and its two use cases cite it.
    std::fs::remove_file(repo.path().join("app/pay-invoice.rb")).unwrap();
    let after = count(router).await;
    assert!(after.contains("8 items"), "{after}");
}

// spec: DBT-14
#[tokio::test]
async fn the_debt_page_of_a_repository_with_no_run_says_to_run_generate() {
    let empty = tempfile::tempdir().unwrap();
    let page = debt(empty.path(), "/debt").await;
    assert!(page.contains("retrodoc generate"), "{page}");
    let (_, fragment) = get_in(None, empty.path(), "/debt", &["HX-Request"]).await;
    assert!(fragment.contains("retrodoc generate"), "{fragment}");
}

// spec: WEB-77, DBT-1
#[tokio::test]
async fn the_debt_page_is_in_french() {
    let repo = generated_repo();
    let (_, page) = get_in(Some("fr"), repo.path(), "/debt", &[]).await;
    for label in [
        "Confiance faible",
        "Non évalué",
        "Mis à part",
        "Périmé",
        "Dette",
    ] {
        assert!(page.contains(label), "{label} missing from {page}");
    }
    for label in ["Low confidence", "Not scored", "Set apart", ">Stale<"] {
        assert!(!page.contains(label), "{label} left in {page}");
    }
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{start} not in {text}"));
    let rest = &text[from..];
    let to = rest.find(end).unwrap_or(rest.len());
    &rest[..to]
}
