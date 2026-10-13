use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;
use crate::contexts::ResolvedContext;
use crate::glossary::{Entity, ModelFile};

fn context(slug: &str, files: &[&str]) -> ResolvedContext {
    ResolvedContext {
        slug: slug.to_string(),
        name: slug.to_uppercase(),
        description: "d".to_string(),
        files: files.iter().map(PathBuf::from).collect(),
        entry_files: Vec::new(),
    }
}

fn entity(name: &str, attributes: usize) -> Entity {
    Entity {
        name: name.to_string(),
        description: String::new(),
        attributes: (0..attributes).map(|i| format!("a{i}")).collect(),
        associations: Vec::new(),
    }
}

fn glossary() -> Glossary {
    let model = |entities: Vec<Entity>| ModelFile {
        content_hash: String::new(),
        entities,
    };
    Glossary {
        models: BTreeMap::from([
            (
                PathBuf::from("a/order.rb"),
                model(vec![entity("Order", 3), entity("Line", 1)]),
            ),
            (
                PathBuf::from("a/shared.rb"),
                model(vec![entity("Order", 3)]),
            ),
            (PathBuf::from("b/user.rb"), model(vec![entity("User", 5)])),
        ]),
        tests: Vec::new(),
    }
}

fn resolved() -> Resolved {
    Resolved {
        contexts: vec![
            context("orders", &["a/order.rb", "a/orders_controller.rb"]),
            context("users", &["b/user.rb"]),
        ],
        support: vec![PathBuf::from("a/shared.rb")],
    }
}

#[test]
fn a_context_is_a_domain_of_its_files_and_the_files_read_for_it() {
    let reads = BTreeMap::from([("orders".to_string(), vec![PathBuf::from("a/shared.rb")])]);

    let map = domains_from_contexts(&resolved(), &reads, &glossary(), &["orders".to_string()]);

    assert_eq!(map.domains.len(), 1);
    let orders = &map.domains[0];
    assert_eq!(orders.slug, "orders");
    assert_eq!(orders.name, "ORDERS");
    assert!(orders.by_paths);
    assert_eq!(orders.resources, Vec::<String>::new());
    assert_eq!(
        orders.paths,
        ["a/order.rb", "a/orders_controller.rb", "a/shared.rb"].map(PathBuf::from)
    );
    assert_eq!(orders.entities, ["Order", "Line"], "once each");
}

#[test]
fn the_domains_follow_the_order_of_the_map_and_only_the_contexts_asked_for() {
    let map = domains_from_contexts(
        &resolved(),
        &BTreeMap::new(),
        &glossary(),
        &["users".to_string(), "orders".to_string()],
    );

    let slugs: Vec<&str> = map.domains.iter().map(|d| d.slug.as_str()).collect();
    assert_eq!(slugs, ["orders", "users"]);
}

#[test]
fn the_vocabulary_of_a_domain_is_its_own_best_connected_entities() {
    let reads = BTreeMap::from([("orders".to_string(), vec![PathBuf::from("a/shared.rb")])]);
    let map = domains_from_contexts(
        &resolved(),
        &reads,
        &glossary(),
        &["orders".to_string(), "users".to_string()],
    );

    let vocabulary = domain_vocabulary(&map, &glossary(), 1);

    assert_eq!(vocabulary["orders"], ["Order"]);
    assert_eq!(vocabulary["users"], ["User"]);
}
