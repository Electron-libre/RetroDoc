//! The actors page, the page of each actor and the link of every page to
//! them, asked of the router
//! without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::artifacts::{DomainCluster, DomainMap};
use retrodoc_core::model::{Actor, ActorKind, BusinessRule, Feature, RuleKind, Step, UseCase};
use retrodoc_docs::GeneratedDocs;
use retrodoc_render::DocLinks;
use tower::ServiceExt;

fn feature(slug: &str, domain: &str, name: &str) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: name.into(),
        description: String::new(),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: None,
    }
}

fn step(actor: &str, kind: ActorKind) -> Step {
    Step {
        order: 1,
        description: "Does it".into(),
        actor: Actor {
            name: actor.into(),
            kind,
        },
        action: String::new(),
        source_refs: Vec::new(),
    }
}

fn use_case(slug: &str, feature: &str, primary: Option<&str>, steps: Vec<Step>) -> UseCase {
    UseCase {
        slug: slug.into(),
        feature_slug: feature.into(),
        name: format!("Use case {slug}"),
        description: String::new(),
        steps,
        entry_points: Vec::new(),
        primary_actor: primary.map(Into::into),
        narrative: None,
        business_language: None,
        diagram_mermaid: None,
        confidence: None,
        narrative_check: None,
    }
}

fn rule(slug: &str, statement: &str, kind: RuleKind, use_cases: &[&str]) -> BusinessRule {
    BusinessRule {
        slug: slug.into(),
        statement: statement.into(),
        kind,
        entities: Vec::new(),
        use_cases: use_cases.iter().map(|u| (*u).into()).collect(),
        evidence: Vec::new(),
        confidence: None,
    }
}

/// Domain `billing`, feature `pay-invoice`: `refund` started by `Manager`,
/// whom no step names, with steps of `clerk` and `Clerk`; `pay-by-card`
/// started by `Clerk`, who also takes a step, with a step of the `Bank`
/// system. Domain `shipping`, feature `ship-order`: `ship` with a step of
/// `Clerk`; feature `audit`: a use case that names no actor. Rules: a
/// permission on `refund`, a limit on `pay-by-card`.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_core::artifacts::save_features(
        root,
        &[
            feature("pay-invoice", "billing", "Pay an invoice"),
            feature("audit", "shipping", "Audit"),
            feature("ship-order", "shipping", "Ship an order"),
        ],
    )
    .unwrap();
    retrodoc_core::artifacts::save_use_cases(
        root,
        &[
            use_case(
                "refund",
                "pay-invoice",
                Some("Manager"),
                vec![
                    step("clerk", ActorKind::Human),
                    step("Clerk", ActorKind::Human),
                ],
            ),
            use_case(
                "pay-by-card",
                "pay-invoice",
                Some("Clerk"),
                vec![
                    step("Clerk", ActorKind::Human),
                    step("Bank", ActorKind::System),
                ],
            ),
            use_case("check", "audit", None, Vec::new()),
            use_case(
                "ship",
                "ship-order",
                None,
                vec![step("Clerk", ActorKind::Human)],
            ),
        ],
    )
    .unwrap();
    retrodoc_core::artifacts::save_rules(
        root,
        &[
            rule(
                "pay-invoice-1",
                "Only a manager refunds",
                RuleKind::Permission,
                &["refund"],
            ),
            rule(
                "pay-invoice-2",
                "A card pays up to a ceiling",
                RuleKind::Limit,
                &["pay-by-card"],
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

async fn get(root: &Path, path: &str) -> (StatusCode, String) {
    let response = retrodoc_web::router(
        retrodoc_web::Project::default(),
        GeneratedDocs::load(root, &[]).unwrap(),
        DocLinks::default(),
        Vec::new(),
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

async fn actors(root: &Path) -> String {
    let (status, page) = get(root, "/actors").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

/// The `<main>` of the page of the actor whose id is `id`.
async fn actor(root: &Path, id: &str) -> String {
    let (status, page) = get(root, &format!("/actors/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{id}: {page}");
    let start = position(&page, "<main");
    page[start..position(&page, "</main>")].to_string()
}

/// The list of the index under the id `id`.
fn index<'a>(page: &'a str, id: &str) -> &'a str {
    let start = page
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("no index {id} in {page}"));
    let rest = &page[start..];
    &rest[..rest.find("</ul>").expect(rest)]
}

/// The item of the index of the actor `id`, which links to its page.
fn entry<'a>(index: &'a str, id: &str) -> &'a str {
    let start = index
        .find(&format!("<li id=\"{id}\"><a href=\"/actors/{id}\">"))
        .unwrap_or_else(|| panic!("no entry {id} in {index}"));
    let rest = &index[start..];
    &rest[..rest.find("</li>").expect(rest)]
}

/// The part of `html` from `start` to the first `end` after it.
fn until<'a>(html: &'a str, start: &str, end: &str) -> &'a str {
    let from = position(html, start);
    &html[from..from + position(&html[from..], end)]
}

/// The text of `html`, its tags removed and its spaces collapsed.
fn text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn position(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("no {needle} in {haystack}"))
}

const PAY_BY_CARD: &str = "href=\"/domains/billing/pay-invoice/pay-by-card\"";
const REFUND: &str = "href=\"/domains/billing/pay-invoice/refund\"";
const SHIP: &str = "href=\"/domains/shipping/ship-order/ship\"";
const BILLING: &str = "<a href=\"/domains/billing\">Billing</a>";
const PAY_AN_INVOICE: &str = "<a href=\"/domains/billing/pay-invoice\">Pay an invoice</a>";
const SHIPPING: &str = "<a href=\"/domains/shipping\">Shipping</a>";
const SHIP_AN_ORDER: &str = "<a href=\"/domains/shipping/ship-order\">Ship an order</a>";
const PERMISSION: &str =
    "<a href=\"/domains/billing/pay-invoice#rule-pay-invoice-1\">Only a manager refunds</a>";

// spec: WEB-52
#[tokio::test]
async fn every_page_links_to_the_actors_page() {
    let repo = generated_repo();
    for path in ["/", "/rules", "/actors", "/actors/actor-clerk", "/unknown"] {
        let (_, page) = get(repo.path(), path).await;
        let header = &page[page.find("<header").unwrap()..page.find("</header>").unwrap()];
        assert!(header.contains("href=\"/actors\""), "{path}: {header}");
    }
}

// spec: WAC-1, WAC-6, WAC-7
#[tokio::test]
async fn the_index_lists_the_people_then_the_systems_by_name_with_their_counts() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    let people = index(&page, "human-actors");
    let positions: Vec<usize> = ["actor-clerk", "actor-clerk-2", "actor-manager"]
        .iter()
        .map(|id| position(people, &format!("href=\"/actors/{id}\"")))
        .collect();
    assert!(positions.is_sorted(), "{positions:?} in {people}");
    for (id, name, counts) in [
        ("actor-clerk", "Clerk", "starts 1 · takes part in 2"),
        ("actor-clerk-2", "clerk", "starts 0 · takes part in 1"),
        ("actor-manager", "Manager", "starts 1 · takes part in 0"),
    ] {
        let entry = entry(people, id);
        assert!(entry.contains(&format!(">{name}</a>")), "{entry}");
        assert!(text(entry).contains(counts), "{id}: {entry}");
    }
    assert!(!people.contains("Bank"), "{people}");
    let systems = index(&page, "system-actors");
    assert!(
        text(entry(systems, "system-bank")).contains("starts 0 · takes part in 1"),
        "{systems}"
    );
    assert!(
        position(&page, "id=\"human-actors\"") < position(&page, "id=\"system-actors\""),
        "{page}"
    );
}

// spec: WAC-1
#[tokio::test]
async fn the_index_lists_no_use_case() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    // The tree of the sidebar lists the use cases on every page.
    let page = &page[position(&page, "<main")..position(&page, "</main>")];
    for link in [
        PAY_BY_CARD,
        REFUND,
        SHIP,
        BILLING,
        PAY_AN_INVOICE,
        PERMISSION,
    ] {
        assert!(!page.contains(link), "{link} in {page}");
    }
    assert!(!page.contains("<section class=\"actor\""), "{page}");
}

// spec: WAC-2
#[tokio::test]
async fn each_actor_of_the_index_has_a_page_headed_by_its_kind_and_counts() {
    let repo = generated_repo();
    for (id, name, kind, counts) in [
        (
            "actor-clerk",
            "Clerk",
            "Person",
            "starts 1 · takes part in 2",
        ),
        (
            "actor-clerk-2",
            "clerk",
            "Person",
            "starts 0 · takes part in 1",
        ),
        (
            "actor-manager",
            "Manager",
            "Person",
            "starts 1 · takes part in 0",
        ),
        (
            "system-bank",
            "Bank",
            "System",
            "starts 0 · takes part in 1",
        ),
    ] {
        let page = actor(repo.path(), id).await;
        let heading = until(&page, "<h1", "</h1>");
        assert!(text(heading).starts_with(name), "{heading}");
        assert!(text(heading).contains(kind), "{heading}");
        let head = &page[..position(&page, "<h2")];
        assert!(text(head).contains(counts), "{id}: {head}");
        assert!(head.contains("href=\"/actors\""), "{head}");
    }
}

// spec: WAC-3
#[tokio::test]
async fn the_page_of_an_actor_lists_its_domains_and_features_with_their_counts() {
    let repo = generated_repo();
    let clerk = actor(repo.path(), "actor-clerk").await;
    let order = [
        position(&clerk, BILLING),
        position(&clerk, PAY_AN_INVOICE),
        position(&clerk, SHIPPING),
        position(&clerk, SHIP_AN_ORDER),
    ];
    assert!(order.is_sorted(), "{order:?} in {clerk}");
    for (start, counts) in [
        (BILLING, "1 feature · 2 use cases"),
        (SHIPPING, "1 feature · 1 use case"),
    ] {
        let heading = until(&clerk, start, "</h2>");
        assert!(text(heading).contains(counts), "{heading}");
    }
    for (start, counts) in [
        (PAY_AN_INVOICE, "starts 1 · takes part in 1"),
        (SHIP_AN_ORDER, "starts 0 · takes part in 1"),
    ] {
        let summary = until(&clerk, start, "</summary>");
        assert!(text(summary).contains(counts), "{summary}");
    }
    assert!(!clerk.contains("Audit"), "{clerk}");
    let manager = actor(repo.path(), "actor-manager").await;
    assert!(manager.contains(PAY_AN_INVOICE), "{manager}");
    assert!(!manager.contains("Shipping"), "{manager}");
}

// spec: WAC-10
#[tokio::test]
async fn the_use_cases_are_folded_under_their_feature_those_started_first() {
    let repo = generated_repo();
    let clerk = actor(repo.path(), "actor-clerk").await;
    let pay = until(&clerk, "<details", "</details>");
    assert!(!pay.contains(" open"), "{pay}");
    // The summary only folds: the link of the feature stays outside it.
    assert!(
        !until(pay, "<summary", "</summary>").contains("<a "),
        "{pay}"
    );
    let (summary, card, refund) = (
        position(pay, "</summary>"),
        position(pay, PAY_BY_CARD),
        position(pay, REFUND),
    );
    // `refund` comes first in the run, but `Clerk` starts `pay-by-card`.
    assert!(summary < card && card < refund, "{pay}");
    assert!(
        until(pay, PAY_BY_CARD, "</li>").contains("class=\"badge\""),
        "{pay}"
    );
    assert!(!until(pay, REFUND, "</li>").contains("badge"), "{pay}");
    assert_eq!(clerk.matches(PAY_BY_CARD).count(), 1, "{clerk}");
    assert!(
        until(&clerk, SHIP_AN_ORDER, "</details>").contains(SHIP),
        "{clerk}"
    );
    let manager = actor(repo.path(), "actor-manager").await;
    assert!(manager.contains(REFUND), "{manager}");
    assert!(!manager.contains(PAY_BY_CARD), "{manager}");
    let bank = actor(repo.path(), "system-bank").await;
    assert!(
        bank.contains(PAY_BY_CARD) && !bank.contains("badge"),
        "{bank}"
    );
}

// spec: WAC-4
#[tokio::test]
async fn the_page_of_an_actor_lists_the_permission_rules_of_its_use_cases_after_them() {
    let repo = generated_repo();
    for id in ["actor-clerk", "actor-clerk-2", "actor-manager"] {
        let page = actor(repo.path(), id).await;
        assert!(page.contains(PERMISSION), "{id}: {page}");
        assert!(
            position(&page, REFUND) < position(&page, PERMISSION),
            "{page}"
        );
        assert!(!page.contains("A card pays up to a ceiling"), "{page}");
    }
    let bank = actor(repo.path(), "system-bank").await;
    assert!(!bank.contains("rule-"), "{bank}");
}

// spec: WAC-H1
#[tokio::test]
async fn the_reader_picks_an_actor_and_sees_what_it_starts_and_takes_part_in() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    let entry = entry(index(&page, "human-actors"), "actor-clerk");
    assert!(
        text(entry).contains("starts 1 · takes part in 2"),
        "{entry}"
    );
    let clerk = actor(repo.path(), "actor-clerk").await;
    assert!(
        text(until(&clerk, BILLING, "</h2>")).contains("1 feature · 2 use cases"),
        "{clerk}"
    );
    assert!(
        text(until(&clerk, PAY_AN_INVOICE, "</summary>")).contains("starts 1 · takes part in 1"),
        "{clerk}"
    );
    let order = [
        position(&clerk, BILLING),
        position(&clerk, PAY_AN_INVOICE),
        position(&clerk, PAY_BY_CARD),
        position(&clerk, REFUND),
        position(&clerk, SHIPPING),
        position(&clerk, SHIP),
        position(&clerk, PERMISSION),
    ];
    assert!(order.is_sorted(), "{order:?} in {clerk}");
}

// spec: WAC-6
#[tokio::test]
async fn a_name_the_steps_give_both_kinds_is_an_actor_of_each_kind() {
    let repo = generated_repo();
    retrodoc_core::artifacts::save_use_cases(
        repo.path(),
        &[
            use_case(
                "pay-by-card",
                "pay-invoice",
                None,
                vec![step("Gateway", ActorKind::Human)],
            ),
            use_case(
                "ship",
                "ship-order",
                Some("Gateway"),
                vec![step("Gateway", ActorKind::System)],
            ),
        ],
    )
    .unwrap();
    let human = actor(repo.path(), "actor-gateway").await;
    assert!(human.contains(PAY_BY_CARD), "{human}");
    // As a primary actor, it starts `ship` in the human group only.
    assert!(until(&human, SHIP, "</li>").contains("badge"), "{human}");
    let system = actor(repo.path(), "system-gateway").await;
    assert!(
        !system.contains(PAY_BY_CARD) && system.contains(SHIP),
        "{system}"
    );
    assert!(!system.contains("badge"), "{system}");
    let page = actors(repo.path()).await;
    assert!(
        text(entry(index(&page, "system-actors"), "system-gateway"))
            .contains("starts 0 · takes part in 1"),
        "{page}"
    );
}

// spec: WAC-7
#[tokio::test]
async fn the_id_of_an_actor_is_the_slug_of_the_name_after_its_kind() {
    let repo = generated_repo();
    retrodoc_core::artifacts::save_use_cases(
        repo.path(),
        &[use_case(
            "pay-by-card",
            "pay-invoice",
            Some("???"),
            vec![step("Payment gateway (Stripe)", ActorKind::System)],
        )],
    )
    .unwrap();
    let page = actors(repo.path()).await;
    entry(
        index(&page, "system-actors"),
        "system-payment-gateway-stripe",
    );
    entry(index(&page, "human-actors"), "actor-actor");
    actor(repo.path(), "system-payment-gateway-stripe").await;
    actor(repo.path(), "actor-actor").await;
}

// spec: WAC-7
#[tokio::test]
async fn the_slug_keeps_the_letters_of_any_script() {
    let repo = generated_repo();
    retrodoc_core::artifacts::save_use_cases(
        repo.path(),
        &[use_case(
            "pay-by-card",
            "pay-invoice",
            Some("Équipe support"),
            Vec::new(),
        )],
    )
    .unwrap();
    let page = actors(repo.path()).await;
    entry(index(&page, "human-actors"), "actor-équipe-support");
    actor(repo.path(), "actor-%C3%A9quipe-support").await;
}

// spec: WAC-6
#[tokio::test]
async fn an_actor_of_a_use_case_whose_feature_is_not_in_the_run_is_left_out() {
    let repo = generated_repo();
    retrodoc_core::artifacts::save_use_cases(
        repo.path(),
        &[
            use_case("ship", "ship-order", Some("Clerk"), Vec::new()),
            use_case("audit-all", "gone", Some("Auditor"), Vec::new()),
        ],
    )
    .unwrap();
    let page = actors(repo.path()).await;
    entry(index(&page, "human-actors"), "actor-clerk");
    assert!(!page.contains("Auditor"), "{page}");
    let (status, _) = get(repo.path(), "/actors/actor-auditor").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// spec: WAC-3
#[tokio::test]
async fn a_domain_the_run_does_not_list_is_named_without_a_link() {
    let repo = generated_repo();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            ..DomainCluster::default()
        }],
    }
    .save(repo.path())
    .unwrap();
    let clerk = actor(repo.path(), "actor-clerk").await;
    assert!(
        text(&clerk).contains("shipping 1 feature · 1 use case"),
        "{clerk}"
    );
    assert!(!clerk.contains("href=\"/domains/shipping\""), "{clerk}");
    assert!(clerk.contains(SHIP), "{clerk}");
}

// spec: WAC-7
#[tokio::test]
async fn an_actor_takes_no_id_of_a_list_of_the_index() {
    let repo = generated_repo();
    retrodoc_core::artifacts::save_use_cases(
        repo.path(),
        &[use_case(
            "pay-by-card",
            "pay-invoice",
            None,
            vec![step("Actors", ActorKind::System)],
        )],
    )
    .unwrap();
    let page = actors(repo.path()).await;
    assert_eq!(page.matches("id=\"system-actors\"").count(), 1, "{page}");
    entry(index(&page, "system-actors"), "system-actors-2");
    actor(repo.path(), "system-actors-2").await;
}

// spec: WAC-8
#[tokio::test]
async fn neither_the_actors_page_nor_the_page_of_an_actor_holds_a_table() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    assert!(!page.contains("<table"), "{page}");
    let clerk = actor(repo.path(), "actor-clerk").await;
    assert!(!clerk.contains("<table"), "{clerk}");
}

// spec: WAC-9
#[tokio::test]
async fn the_page_of_an_unknown_actor_answers_404() {
    let repo = generated_repo();
    for id in ["actor-nobody", "actor-bank", "clerk"] {
        let (status, page) = get(repo.path(), &format!("/actors/{id}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {page}");
        assert!(page.contains("Not found"), "{page}");
    }
    actor(repo.path(), "system-bank").await;
}

// spec: WAC-9
#[tokio::test]
async fn with_no_run_the_page_of_an_actor_says_to_run_generate() {
    let dir = tempfile::tempdir().unwrap();
    let (_, page) = get(dir.path(), "/actors/actor-clerk").await;
    assert!(page.contains("retrodoc generate"), "{page}");
}

// spec: WAC-1
#[tokio::test]
async fn a_run_with_no_actor_says_none_was_found() {
    let repo = generated_repo();
    retrodoc_core::artifacts::save_use_cases(
        repo.path(),
        &[use_case("check", "audit", None, Vec::new())],
    )
    .unwrap();
    let page = actors(repo.path()).await;
    assert!(page.contains("No actor was found"), "{page}");
    assert!(!page.contains("actor-index"), "{page}");
    assert!(!page.contains("id=\"human-actors\""), "{page}");
}

// spec: WAC-1
#[tokio::test]
async fn with_no_run_the_actors_page_says_to_run_generate() {
    let dir = tempfile::tempdir().unwrap();
    let (status, page) = get(dir.path(), "/actors").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("retrodoc generate"), "{page}");
}
