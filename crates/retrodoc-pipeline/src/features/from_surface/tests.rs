use super::*;

use crate::entry_points::{EntryFile, EntryKind};
use crate::testing::FakeLlm;
use retrodoc_llm::{CompletionRequest, LlmError};

fn entry(name: &str, resource: &str) -> EntryPoint {
    EntryPoint {
        kind: EntryKind::HttpRoute,
        name: name.to_string(),
        verb: "get".to_string(),
        resource: resource.to_string(),
        description: format!("does {name}"),
        outputs: Vec::new(),
    }
}

fn entry_points(files: &[(&str, Vec<EntryPoint>)]) -> EntryPoints {
    EntryPoints {
        files: files
            .iter()
            .map(|(path, entries)| {
                (
                    PathBuf::from(path),
                    EntryFile {
                        content_hash: "h".to_string(),
                        entry_points: entries.clone(),
                    },
                )
            })
            .collect(),
    }
}

fn domain(slug: &str, resources: &[&str]) -> DomainCluster {
    DomainCluster {
        slug: slug.to_string(),
        name: slug.to_string(),
        description: "d".to_string(),
        resources: resources.iter().map(ToString::to_string).collect(),
        ..DomainCluster::default()
    }
}

/// `orders.rb` serves two resources of two domains; `routes.rb` one.
fn fixture() -> (DomainMap, EntryPoints) {
    let domains = DomainMap {
        domains: vec![
            domain("orders", &["order"]),
            domain("routing", &["route"]),
            domain(UNCATEGORIZED_SLUG, &["misc"]),
        ],
    };
    let entries = entry_points(&[
        (
            "orders.rb",
            vec![
                entry("GET /orders", "order"),
                entry("POST /orders", "order"),
                entry("GET /routes", "route"),
            ],
        ),
        ("misc.rb", vec![entry("GET /ping", "misc")]),
    ]);
    (domains, entries)
}

async fn run(
    dir: &Path,
    domains: &DomainMap,
    entries: &EntryPoints,
    llm: &FakeLlm,
) -> Vec<Feature> {
    build_features_from_surface(
        dir,
        domains,
        entries,
        &Surface::default(),
        &ProductBrief::default(),
        &Evidence::default(),
        llm,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn groups_the_entry_points_of_each_domain_and_skips_uncategorized() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let llm = FakeLlm::replying(|n, _| {
        Ok(if n == 0 {
            r#"{"features":[{"slug":"Place order","name":"Place order","description":"d",
                "entry_points":["orders.rb#POST /orders","orders.rb#GET /orders"]}]}"#
        } else {
            r#"{"features":[{"slug":"list-routes","name":"List routes","description":"d",
                "entry_points":["orders.rb#GET /routes"]}]}"#
        }
        .to_string())
    });

    let features = run(dir.path(), &domains, &entries, &llm).await;

    assert_eq!(
        llm.calls(),
        2,
        "one call per domain, none for uncategorized"
    );
    assert_eq!(features.len(), 2);
    assert_eq!(features[0].slug, "place-order");
    assert_eq!(features[0].domain_slug, "orders");
    assert_eq!(
        features[0].entry_points,
        vec!["orders.rb#POST /orders", "orders.rb#GET /orders"]
    );
    assert_eq!(features[0].source_paths, vec!["orders.rb"]);
    assert_eq!(features[1].entry_points, vec!["orders.rb#GET /routes"]);
    assert_eq!(load_features(dir.path()).unwrap().len(), 2);
}

#[tokio::test]
async fn an_entry_point_goes_to_one_feature_and_unknown_ids_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    // The routes entry point is not an order one; the second feature only repeats an id.
    let llm = FakeLlm::answering(
        r#"{"features":[
            {"slug":"a","name":"A","description":"d",
             "entry_points":["orders.rb#GET /orders","orders.rb#GET /routes","ghost#x"]},
            {"slug":"a","name":"B","description":"d","entry_points":["orders.rb#GET /orders"]},
            {"slug":"c","name":"C","description":"d","entry_points":["orders.rb#POST /orders"]}
        ]}"#,
    );
    let only_orders = DomainMap {
        domains: vec![domains.domains[0].clone()],
    };

    let features = run(dir.path(), &only_orders, &entries, &llm).await;

    assert_eq!(features.len(), 2, "the repeat leaves feature B empty");
    assert_eq!(features[0].entry_points, vec!["orders.rb#GET /orders"]);
    assert_eq!(features[1].slug, "c");
    assert_eq!(features[1].entry_points, vec!["orders.rb#POST /orders"]);
}

#[tokio::test]
async fn a_rerun_on_the_same_surface_makes_no_call_and_a_changed_entry_point_one() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let answer = |n: usize, _: &CompletionRequest| -> Result<String, LlmError> {
        Ok(if n == 0 {
            r#"{"features":[{"slug":"orders","name":"O","description":"d",
                "entry_points":["orders.rb#GET /orders","orders.rb#POST /orders"]}]}"#
        } else {
            r#"{"features":[{"slug":"routes","name":"R","description":"d",
                "entry_points":["orders.rb#GET /routes"]}]}"#
        }
        .to_string())
    };
    let first = FakeLlm::replying(answer);
    let before = run(dir.path(), &domains, &entries, &first).await;

    let second = FakeLlm::replying(answer);
    let again = run(dir.path(), &domains, &entries, &second).await;
    assert_eq!(second.calls(), 0);
    assert_eq!(again.len(), before.len());

    let mut changed = entries.clone();
    changed
        .files
        .get_mut(Path::new("orders.rb"))
        .unwrap()
        .entry_points[2]
        .description = "now does something else".to_string();
    let third = FakeLlm::replying(|_, _| {
        Ok(
            r#"{"features":[{"slug":"routes","name":"R","description":"d",
            "entry_points":["orders.rb#GET /routes"]}]}"#
                .to_string(),
        )
    });
    let redone = run(dir.path(), &domains, &changed, &third).await;
    assert_eq!(third.calls(), 1, "only the routing domain is asked again");
    assert_eq!(redone.len(), 2);
}

#[tokio::test]
async fn the_prompt_shows_ids_and_only_the_entry_points_of_the_domain() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let llm = FakeLlm::answering(r#"{"features":[]}"#);

    run(dir.path(), &domains, &entries, &llm).await;

    let prompts = llm.prompts();
    assert!(prompts[0].contains("orders.rb#GET /orders"));
    assert!(!prompts[0].contains("GET /routes"));
    assert!(prompts[1].contains("orders.rb#GET /routes"));
}
