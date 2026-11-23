//! The actors page and the link of every page to it, asked of the router
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

/// The list of the index under the id `id`.
fn index<'a>(page: &'a str, id: &str) -> &'a str {
    let start = page
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("no index {id} in {page}"));
    let rest = &page[start..];
    &rest[..rest.find("</ul>").expect(rest)]
}

/// The item of the index that links to the section `id`.
fn entry<'a>(index: &'a str, id: &str) -> &'a str {
    let start = index
        .find(&format!("href=\"#{id}\""))
        .unwrap_or_else(|| panic!("no entry #{id} in {index}"));
    let rest = &index[start..];
    &rest[..rest.find("</li>").expect(rest)]
}

/// The section of the actor whose id is `id`.
fn actor<'a>(page: &'a str, id: &str) -> &'a str {
    let start = page
        .find(&format!("<section class=\"actor\" id=\"{id}\""))
        .unwrap_or_else(|| panic!("no section {id} in {page}"));
    let rest = &page[start..];
    &rest[..rest.find("</section>").expect(rest)]
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
const PERMISSION: &str =
    "<a href=\"/domains/billing/pay-invoice#rule-pay-invoice-1\">Only a manager refunds</a>";

// spec: WEB-52
#[tokio::test]
async fn every_page_links_to_the_actors_page() {
    let repo = generated_repo();
    for path in ["/", "/rules", "/actors", "/unknown"] {
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
        .map(|id| position(people, &format!("href=\"#{id}\"")))
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

// spec: WAC-2
#[tokio::test]
async fn a_section_per_actor_in_the_order_of_the_index_headed_by_its_counts() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    let index_end = position(&page, "id=\"system-actors\"");
    let positions: Vec<usize> = [
        "actor-clerk",
        "actor-clerk-2",
        "actor-manager",
        "system-bank",
    ]
    .iter()
    .map(|id| position(&page, &format!("<section class=\"actor\" id=\"{id}\"")))
    .collect();
    assert!(positions.is_sorted(), "{positions:?}");
    assert!(index_end < positions[0], "{page}");
    let clerk = actor(&page, "actor-clerk");
    let heading = &clerk[..position(clerk, "</h2>")];
    assert!(text(heading).contains("Clerk"), "{heading}");
    let head = &clerk[..position(clerk, "<h3")];
    assert!(text(head).contains("starts 1 · takes part in 2"), "{head}");
}

// spec: WAC-3
#[tokio::test]
async fn a_section_lists_the_use_cases_by_domain_and_feature_those_started_first() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    let clerk = actor(&page, "actor-clerk");
    let billing = position(clerk, "<a href=\"/domains/billing\">Billing</a>");
    let pay = position(
        clerk,
        "<a href=\"/domains/billing/pay-invoice\">Pay an invoice</a>",
    );
    let shipping = position(clerk, "<a href=\"/domains/shipping\">Shipping</a>");
    let ship_order = position(
        clerk,
        "<a href=\"/domains/shipping/ship-order\">Ship an order</a>",
    );
    let (card, refund, ship) = (
        position(clerk, PAY_BY_CARD),
        position(clerk, REFUND),
        position(clerk, SHIP),
    );
    // `refund` comes first in the run, but `Clerk` starts `pay-by-card`.
    let order = [billing, pay, card, refund, shipping, ship_order, ship];
    assert!(order.is_sorted(), "{order:?} in {clerk}");
    assert_eq!(clerk.matches(PAY_BY_CARD).count(), 1, "{clerk}");
    let card_item = &clerk[card..card + position(&clerk[card..], "</li>")];
    assert!(card_item.contains("class=\"badge\""), "{card_item}");
    let refund_item = &clerk[refund..refund + position(&clerk[refund..], "</li>")];
    assert!(!refund_item.contains("badge"), "{refund_item}");
    assert!(!clerk.contains("Audit"), "{clerk}");
    let manager = actor(&page, "actor-manager");
    assert!(manager.contains(REFUND), "{manager}");
    assert!(!manager.contains("Shipping"), "{manager}");
    assert!(!manager.contains(PAY_BY_CARD), "{manager}");
    let bank = actor(&page, "system-bank");
    assert!(
        bank.contains(PAY_BY_CARD) && !bank.contains("badge"),
        "{bank}"
    );
}

// spec: WAC-4
#[tokio::test]
async fn a_section_lists_the_permission_rules_of_its_use_cases_after_them() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    for id in ["actor-clerk", "actor-clerk-2", "actor-manager"] {
        let section = actor(&page, id);
        assert!(section.contains(PERMISSION), "{id}: {section}");
        assert!(
            position(section, REFUND) < position(section, PERMISSION),
            "{section}"
        );
    }
    let bank = actor(&page, "system-bank");
    assert!(!bank.contains("rule-"), "{bank}");
    assert!(!page.contains("A card pays up to a ceiling"), "{page}");
}

// spec: WAC-H1
#[tokio::test]
async fn the_reader_sees_what_one_actor_starts_and_takes_part_in() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    let entry = entry(index(&page, "human-actors"), "actor-clerk");
    assert!(
        text(entry).contains("starts 1 · takes part in 2"),
        "{entry}"
    );
    let clerk = actor(&page, "actor-clerk");
    let order = [
        position(clerk, "Billing"),
        position(clerk, "Pay an invoice"),
        position(clerk, PAY_BY_CARD),
        position(clerk, REFUND),
        position(clerk, "Shipping"),
        position(clerk, SHIP),
        position(clerk, PERMISSION),
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
    let page = actors(repo.path()).await;
    let human = actor(&page, "actor-gateway");
    assert!(human.contains(PAY_BY_CARD), "{human}");
    // As a primary actor, it starts `ship` in the human section only.
    let ship = position(human, SHIP);
    assert!(
        human[ship..ship + position(&human[ship..], "</li>")].contains("badge"),
        "{human}"
    );
    let system = actor(&page, "system-gateway");
    assert!(
        !system.contains(PAY_BY_CARD) && system.contains(SHIP),
        "{system}"
    );
    assert!(!system.contains("badge"), "{system}");
    assert!(
        text(entry(index(&page, "system-actors"), "system-gateway"))
            .contains("starts 0 · takes part in 1"),
        "{page}"
    );
}

// spec: WAC-7
#[tokio::test]
async fn the_id_of_a_section_is_the_slug_of_the_name_after_its_kind() {
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
    actor(&page, "system-payment-gateway-stripe");
    actor(&page, "actor-actor");
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
    actor(&page, "system-actors-2");
}

// spec: WAC-8
#[tokio::test]
async fn the_actors_page_holds_no_table() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    assert!(!page.contains("<table"), "{page}");
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
    assert!(!page.contains("class=\"actor\""), "{page}");
}

// spec: WAC-1
#[tokio::test]
async fn with_no_run_the_actors_page_says_to_run_generate() {
    let dir = tempfile::tempdir().unwrap();
    let (status, page) = get(dir.path(), "/actors").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("retrodoc generate"), "{page}");
}
