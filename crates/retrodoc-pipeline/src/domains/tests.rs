use super::*;

use crate::brief::ProductBrief;
use crate::surface::Surface;
use crate::testing::FakeLlm;

/// A provider that panics if called, to assert a fast path never reaches
/// the LLM.
fn never_called() -> FakeLlm {
    FakeLlm::replying(|_, _| panic!("LLM should not have been called"))
}

#[test]
fn flags_domains_named_after_a_technical_layer() {
    let cluster = |slug: &str| DomainCluster {
        slug: slug.to_string(),
        name: slug.to_string(),
        description: String::new(),
        paths: Vec::new(),
        sub_domains: Vec::new(),
        ..DomainCluster::default()
    };
    let map = DomainMap {
        domains: vec![
            cluster("presentation-layer"),
            cluster("contract-signing"),
            cluster("services"),
            cluster("uncategorized"),
        ],
    };
    assert_eq!(
        layer_named_domains(&map),
        vec!["presentation-layer", "services"]
    );
}

fn surface_of(entry_points: &str, glossary: &str) -> Surface {
    let entry_points: crate::entry_points::EntryPoints =
        serde_yaml::from_str(entry_points).unwrap();
    let glossary: crate::glossary::Glossary = serde_yaml::from_str(glossary).unwrap();
    Surface::new(&glossary, &entry_points)
}

/// Three resources (order, invoice, webhook) and three entities, in files of their own.
fn shop_surface() -> Surface {
    surface_of(
        "files:\n\
         \x20 app/orders_controller.rb:\n    content_hash: h\n    entry_points:\n\
         \x20   - {kind: http_route, name: POST /orders, verb: create, resource: order}\n\
         \x20 app/invoices_controller.rb:\n    content_hash: h\n    entry_points:\n\
         \x20   - {kind: http_route, name: GET /invoices, verb: list, resource: invoice}\n\
         \x20 app/hooks.rb:\n    content_hash: h\n    entry_points:\n\
         \x20   - {kind: webhook, name: POST /hooks, verb: receive, resource: webhook}\n",
        "models:\n\
         \x20 app/order.rb:\n    content_hash: h\n    entities:\n    - {name: Order, description: A customer order}\n\
         \x20 app/invoice.rb:\n    content_hash: h\n    entities:\n    - {name: Invoice, description: A bill}\n\
         \x20 app/customer.rb:\n    content_hash: h\n    entities:\n    - {name: Customer, description: Who buys}\n\
         tests: []\n",
    )
}

const SHOP_ANSWER: &str = r#"{"domains":[
    {"slug":"ordering","name":"Ordering","description":"Placing orders",
     "resources":["order","ghost"],"entities":["Order","Customer"]},
    {"slug":"billing","name":"Billing","description":"Invoicing",
     "resources":["invoice","order"],"entities":["Invoice","Order"]}]}"#;

#[tokio::test]
async fn domains_from_the_surface_hold_resources_and_entities_and_their_files() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(SHOP_ANSWER);

    let map =
        build_domains_from_surface(dir.path(), &shop_surface(), &ProductBrief::default(), &llm)
            .await
            .unwrap();

    let slugs: Vec<&str> = map.domains.iter().map(|d| d.slug.as_str()).collect();
    assert_eq!(slugs, ["ordering", "billing", UNCATEGORIZED_SLUG]);
    let (ordering, billing, rest) = (&map.domains[0], &map.domains[1], &map.domains[2]);
    // "ghost" is no resource of the surface; "Order" and the order resource stay with the first domain.
    assert_eq!(ordering.resources, ["order"]);
    assert_eq!(ordering.entities, ["Order", "Customer"]);
    assert_eq!(billing.resources, ["invoice"]);
    assert_eq!(billing.entities, ["Invoice"]);
    // What no domain claimed is kept, not lost.
    assert_eq!(rest.resources, ["webhook"]);
    assert_eq!(rest.entities, Vec::<String>::new());
    // Files follow: the files of the resources, then the home file of the entities.
    let paths = |d: &DomainCluster| -> Vec<String> {
        d.paths.iter().map(|p| p.display().to_string()).collect()
    };
    assert_eq!(
        paths(ordering),
        [
            "app/customer.rb",
            "app/order.rb",
            "app/orders_controller.rb"
        ]
    );
    assert_eq!(
        paths(billing),
        ["app/invoice.rb", "app/invoices_controller.rb"]
    );
    assert_eq!(paths(rest), ["app/hooks.rb"]);
    assert!(ordering.sub_domains.is_empty());

    assert_eq!(llm.calls(), 1);
    let prompt = &llm.prompts()[0];
    assert!(prompt.contains("order") && prompt.contains("webhook") && prompt.contains("Customer"));
    assert!(
        !prompt.contains("app/orders_controller.rb"),
        "a file path made the model answer files: {prompt}"
    );
    let saved = DomainMap::load(dir.path()).unwrap();
    assert_eq!(saved.domains.len(), 3);
}

#[tokio::test]
async fn the_surface_domains_are_reused_while_the_surface_and_the_model_are_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(SHOP_ANSWER);
    let surface = shop_surface();
    let brief = ProductBrief::default();

    let first = build_domains_from_surface(dir.path(), &surface, &brief, &llm)
        .await
        .unwrap();
    let again = build_domains_from_surface(dir.path(), &surface, &brief, &llm)
        .await
        .unwrap();

    assert_eq!(llm.calls(), 1);
    assert_eq!(first.domains.len(), again.domains.len());

    // Another entry point on a known resource changes what the domains are derived from.
    let more = surface_of(
        "files:\n  app/orders_controller.rb:\n    content_hash: h\n    entry_points:\n\
         \x20   - {kind: http_route, name: POST /orders, verb: create, resource: order}\n\
         \x20   - {kind: http_route, name: DELETE /orders, verb: cancel, resource: order}\n",
        "models: {}\ntests: []\n",
    );
    build_domains_from_surface(dir.path(), &more, &brief, &llm)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn an_empty_surface_gives_no_domain_and_asks_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let map = build_domains_from_surface(
        dir.path(),
        &Surface::default(),
        &ProductBrief::default(),
        &never_called(),
    )
    .await
    .unwrap();
    assert!(map.domains.is_empty());
}

#[tokio::test]
async fn a_domain_the_model_calls_uncategorized_is_the_one_bucket_the_leftovers_join() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(
        r#"{"domains":[
            {"slug":"uncategorized","name":"Other","description":"x","resources":["order"],"entities":[]},
            {"slug":"billing","name":"Billing","description":"y","resources":["invoice"],"entities":["Invoice"]}]}"#,
    );

    let map =
        build_domains_from_surface(dir.path(), &shop_surface(), &ProductBrief::default(), &llm)
            .await
            .unwrap();

    let slugs: Vec<&str> = map.domains.iter().map(|d| d.slug.as_str()).collect();
    assert_eq!(slugs, [UNCATEGORIZED_SLUG, "billing"]);
    // What the model put there plus what nobody claimed (the webhook, Customer, Order), in the order of the surface.
    assert_eq!(map.domains[0].resources, ["order", "webhook"]);
    assert_eq!(map.domains[0].entities, ["Customer", "Order"]);
}
