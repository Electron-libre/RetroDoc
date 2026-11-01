//! The actors page and the link of every page to it, asked of the router
//! without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{Actor, ActorKind, BusinessRule, Feature, RuleKind, Step, UseCase};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use retrodoc_render::DocLinks;
use tower::ServiceExt;

fn feature(slug: &str, name: &str) -> Feature {
    Feature {
        back_office: false,
        slug: slug.into(),
        domain_slug: "billing".into(),
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

/// `pay-invoice`: `pay-by-card` started by `Clerk`, who also takes a step,
/// with a step of the `Bank` system; `refund` started by `Manager`, whom no
/// step names, with a step of `clerk`. `ship-order`: `ship` with a step of
/// `Clerk`. `audit` has a use case that names no actor. Rules: a permission
/// on `refund`, a limit on `pay-by-card`.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(
        root,
        &[
            feature("pay-invoice", "Pay an invoice"),
            feature("audit", "Audit"),
            feature("ship-order", "Ship an order"),
        ],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(
        root,
        &[
            use_case(
                "pay-by-card",
                "pay-invoice",
                Some("Clerk"),
                vec![
                    step("Clerk", ActorKind::Human),
                    step("Bank", ActorKind::System),
                ],
            ),
            use_case(
                "refund",
                "pay-invoice",
                Some("Manager"),
                vec![step("clerk", ActorKind::Human)],
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
    retrodoc_pipeline::save_rules(
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
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            ..DomainCluster::default()
        }],
    }
    .save(root)
    .unwrap();
    dir
}

async fn get(root: &Path, path: &str) -> (StatusCode, String) {
    let response = retrodoc_web::router(
        GeneratedDocs::load(root, &[]),
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

/// The section of the human or the system actors, up to the next one.
fn section<'a>(page: &'a str, id: &str) -> &'a str {
    let start = page
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("no section {id} in {page}"));
    let rest = &page[start..];
    &rest[..rest.find("</table>").expect(rest)]
}

/// The row of the actor `name`.
fn row<'a>(section: &'a str, name: &str) -> &'a str {
    let start = section
        .find(&format!("<th scope=\"row\">{name}</th>"))
        .unwrap_or_else(|| panic!("no row {name} in {section}"));
    let rest = &section[start..];
    &rest[..rest.find("</tr>").unwrap()]
}

/// The cells of a row, after its heading.
fn cells(row: &str) -> Vec<&str> {
    row.split("<td").skip(1).collect()
}

const PAY_BY_CARD: &str = "href=\"/domains/billing/pay-invoice/pay-by-card\"";
const REFUND: &str = "href=\"/domains/billing/pay-invoice/refund\"";
const SHIP: &str = "href=\"/domains/billing/ship-order/ship\"";

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

// spec: WEB-57
#[tokio::test]
async fn the_actors_are_grouped_by_exact_name_humans_then_systems_by_name() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    let human = section(&page, "human-actors");
    let positions: Vec<usize> = ["Clerk", "clerk", "Manager"]
        .iter()
        .map(|name| human.find(&format!(">{name}</th>")).expect(name))
        .collect();
    assert!(positions.is_sorted(), "{positions:?} in {human}");
    assert!(!human.contains(">Bank</th>"), "{human}");
    let system = section(&page, "system-actors");
    assert!(system.contains("<th scope=\"row\">Bank</th>"), "{system}");
    assert!(
        page.find("id=\"human-actors\"") < page.find("id=\"system-actors\""),
        "{page}"
    );
}

// spec: WEB-58
#[tokio::test]
async fn a_column_per_feature_that_names_an_actor_in_the_order_of_the_run() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    let human = section(&page, "human-actors");
    let head = &human[..human.find("</thead>").unwrap()];
    let pay = head
        .find("<a href=\"/domains/billing/pay-invoice\">Pay an invoice</a>")
        .expect(head);
    let ship = head
        .find("<a href=\"/domains/billing/ship-order\">Ship an order</a>")
        .expect(head);
    assert!(pay < ship, "{head}");
    assert!(!head.contains("Audit"), "{head}");
}

// spec: WEB-59
#[tokio::test]
async fn a_cell_links_the_use_cases_the_actor_starts_or_takes_part_in() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    let human = section(&page, "human-actors");
    let clerk = cells(row(human, "Clerk"));
    assert_eq!(clerk.len(), 3, "{clerk:?}");
    assert_eq!(clerk[0].matches(PAY_BY_CARD).count(), 1, "{}", clerk[0]);
    assert!(clerk[0].contains("starts"), "{}", clerk[0]);
    assert!(!clerk[0].contains(REFUND), "{}", clerk[0]);
    assert!(clerk[1].contains(SHIP), "{}", clerk[1]);
    assert!(!clerk[1].contains("starts"), "{}", clerk[1]);
    let manager = cells(row(human, "Manager"));
    assert!(manager[0].contains(REFUND), "{}", manager[0]);
    assert!(manager[0].contains("starts"), "{}", manager[0]);
    assert!(!manager[1].contains("href="), "{}", manager[1]);
    let lower = cells(row(human, "clerk"));
    assert!(lower[0].contains(REFUND), "{}", lower[0]);
    assert!(!lower[0].contains("starts"), "{}", lower[0]);
    let bank = cells(row(section(&page, "system-actors"), "Bank"));
    assert!(bank[0].contains(PAY_BY_CARD), "{}", bank[0]);
}

// spec: WEB-60
#[tokio::test]
async fn each_actor_lists_the_permission_rules_of_its_use_cases() {
    let repo = generated_repo();
    let page = actors(repo.path()).await;
    let human = section(&page, "human-actors");
    let permission =
        "<a href=\"/domains/billing/pay-invoice#rule-pay-invoice-1\">Only a manager refunds</a>";
    assert!(row(human, "Manager").contains(permission), "{human}");
    assert!(row(human, "clerk").contains(permission), "{human}");
    assert!(!row(human, "Clerk").contains("Only a manager"), "{human}");
    assert!(!page.contains("A card pays up to a ceiling"), "{page}");
}

// spec: WEB-57
#[tokio::test]
async fn a_name_the_steps_give_both_kinds_is_a_row_under_each() {
    let repo = generated_repo();
    retrodoc_pipeline::save_use_cases(
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
    let human = cells(row(section(&page, "human-actors"), "Gateway"));
    assert!(
        human[0].contains(PAY_BY_CARD) && !human[0].contains("starts"),
        "{human:?}"
    );
    // As a primary actor, it starts `ship` in the human row only.
    assert!(
        human[1].contains(SHIP) && human[1].contains("starts"),
        "{human:?}"
    );
    let system = cells(row(section(&page, "system-actors"), "Gateway"));
    assert!(
        !system[0].contains(PAY_BY_CARD) && system[1].contains(SHIP),
        "{system:?}"
    );
    assert!(!system[1].contains("starts"), "{system:?}");
}

// spec: WEB-57
#[tokio::test]
async fn a_run_with_no_actor_says_none_was_found() {
    let repo = generated_repo();
    retrodoc_pipeline::save_use_cases(repo.path(), &[use_case("check", "audit", None, Vec::new())])
        .unwrap();
    let page = actors(repo.path()).await;
    assert!(page.contains("No actor was found"), "{page}");
    assert!(!page.contains("<table"), "{page}");
}

// spec: WEB-57
#[tokio::test]
async fn with_no_run_the_actors_page_says_to_run_generate() {
    let dir = tempfile::tempdir().unwrap();
    let (status, page) = get(dir.path(), "/actors").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("retrodoc generate"), "{page}");
}
