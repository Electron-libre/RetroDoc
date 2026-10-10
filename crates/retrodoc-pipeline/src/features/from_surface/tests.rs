use super::*;

use crate::behaviour_coverage::BehaviourCoverage;
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
        3,
        "one call per domain, none for uncategorized, then one to place `GET /ping` (held only by \
         the uncategorized domain; this canned answer places nothing)"
    );
    assert_eq!(
        BehaviourCoverage::load(dir.path())
            .unwrap()
            .unplaced_entry_points,
        vec!["misc.rb#GET /ping"]
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

    // Each domain is asked twice, since the canned answer holds no feature.
    let prompts = llm.prompts();
    assert!(prompts[0].contains("E1 — "));
    assert!(prompts[0].contains("\"GET /orders\" (orders.rb)"));
    assert!(prompts[0].contains("(orders.rb)"));
    assert!(!prompts[0].contains("GET /routes"));
    assert!(prompts[2].contains("\"GET /routes\" (orders.rb)"));
}

/// Routes domain answered by feature `routes`; the order domain claims only `GET /orders`.
fn one_left_out(n: usize, request: &CompletionRequest) -> String {
    let prompt = &request.messages.last().unwrap().content;
    if prompt.contains("Entry points to place") {
        r#"{"placements":[{"entry_point":"orders.rb#POST /orders","feature":"orders"},
            {"entry_point":"misc.rb#GET /ping","feature":"ghost"},
            {"entry_point":"nowhere#x","feature":"orders"}]}"#
            .to_string()
    } else if n == 0 {
        r#"{"features":[{"slug":"orders","name":"O","description":"d",
            "entry_points":["orders.rb#GET /orders"]}]}"#
            .to_string()
    } else {
        r#"{"features":[{"slug":"routes","name":"R","description":"d",
            "entry_points":["orders.rb#GET /routes"]}]}"#
            .to_string()
    }
}

#[tokio::test]
async fn the_entry_points_no_feature_claimed_are_placed_or_listed_as_unplaced() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let llm = FakeLlm::replying(|n, r| Ok(one_left_out(n, r)));

    let features = run(dir.path(), &domains, &entries, &llm).await;

    // 2 domain calls + 1 placement call; `GET /ping` (the uncategorized domain) and
    // `POST /orders` were left out; the ghost feature and the unknown id are dropped.
    assert_eq!(llm.calls(), 3);
    assert_eq!(
        features[0].entry_points,
        vec!["orders.rb#GET /orders", "orders.rb#POST /orders"]
    );
    let coverage = BehaviourCoverage::load(dir.path()).unwrap();
    assert_eq!(coverage.unplaced_entry_points, vec!["misc.rb#GET /ping"]);
    // The placement is saved with the features.
    let saved = load_features(dir.path()).unwrap();
    assert_eq!(saved[0].entry_points.len(), 2);
}

#[tokio::test]
async fn a_rerun_makes_no_placement_call_and_keeps_the_unplaced_list() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    run(
        dir.path(),
        &domains,
        &entries,
        &FakeLlm::replying(|n, r| Ok(one_left_out(n, r))),
    )
    .await;

    let again = FakeLlm::replying(|n, r| Ok(one_left_out(n, r)));
    let features = run(dir.path(), &domains, &entries, &again).await;

    assert_eq!(again.calls(), 0);
    assert_eq!(features[0].entry_points.len(), 2);
    assert_eq!(
        BehaviourCoverage::load(dir.path())
            .unwrap()
            .unplaced_entry_points,
        vec!["misc.rb#GET /ping"]
    );
}

#[tokio::test]
async fn with_every_entry_point_claimed_there_is_no_placement_call() {
    let dir = tempfile::tempdir().unwrap();
    let domains = DomainMap {
        domains: vec![domain("orders", &["order"])],
    };
    let entries = entry_points(&[("orders.rb", vec![entry("GET /orders", "order")])]);
    let llm = FakeLlm::answering(
        r#"{"features":[{"slug":"o","name":"O","description":"d",
            "entry_points":["orders.rb#GET /orders"]}]}"#,
    );

    run(dir.path(), &domains, &entries, &llm).await;

    assert_eq!(llm.calls(), 1);
    assert_eq!(
        BehaviourCoverage::load(dir.path())
            .unwrap()
            .unplaced_entry_points,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn a_bare_entry_point_name_resolves_when_it_is_unique_in_the_domain() {
    let dir = tempfile::tempdir().unwrap();
    let domains = DomainMap {
        domains: vec![domain("orders", &["order"])],
    };
    let entries = entry_points(&[(
        "orders.rb",
        vec![entry("#add_order", "order"), entry("#clear", "order")],
    )]);
    // The model dropped the file prefix, and the leading `#` of one name.
    let llm = FakeLlm::answering(
        r##"{"features":[{"slug":"o","name":"O","description":"d",
            "entry_points":["#add_order","clear"]}]}"##,
    );

    let features = run(dir.path(), &domains, &entries, &llm).await;

    assert_eq!(
        features[0].entry_points,
        vec!["orders.rb##add_order", "orders.rb##clear"]
    );
}

#[tokio::test]
async fn a_domain_answered_with_no_usable_feature_is_asked_once_more() {
    let dir = tempfile::tempdir().unwrap();
    let domains = DomainMap {
        domains: vec![domain("orders", &["order"])],
    };
    let entries = entry_points(&[("orders.rb", vec![entry("GET /orders", "order")])]);
    let llm = FakeLlm::replying(|n, _| {
        Ok(if n == 0 {
            r#"{"features":[{"slug":"o","name":"O","description":"d","entry_points":["ghost"]}]}"#
        } else {
            r#"{"features":[{"slug":"o","name":"O","description":"d",
                "entry_points":["orders.rb#GET /orders"]}]}"#
        }
        .to_string())
    });

    let features = run(dir.path(), &domains, &entries, &llm).await;

    assert_eq!(llm.calls(), 2);
    assert_eq!(features.len(), 1);
}

#[test]
fn an_entry_point_held_by_two_features_stays_with_the_domain_that_owns_it() {
    let (domains, entries) = fixture();
    let units: Vec<(&DomainCluster, Vec<Entry<'_>>)> = domains
        .domains
        .iter()
        .map(|d| (d, domain_entries(d, &entries)))
        .collect();
    let held = |slug: &str, domain: &str, ids: &[&str]| Feature {
        slug: slug.to_string(),
        domain_slug: domain.to_string(),
        sub_domain_slug: None,
        name: slug.to_string(),
        description: "d".to_string(),
        source_paths: vec!["stale.rb".to_string()],
        entry_points: ids.iter().map(ToString::to_string).collect(),
        confidence: None,
    };
    // `POST /orders` was placed in the routing feature, for lack of a feature in its own domain;
    // the order domain claims it now. A routing feature left with nothing is dropped.
    let mut features = vec![
        held(
            "routes",
            "routing",
            &["orders.rb#GET /routes", "orders.rb#POST /orders"],
        ),
        held("only-order", "routing", &["orders.rb#GET /orders"]),
        held(
            "orders",
            "orders",
            &["orders.rb#GET /orders", "orders.rb#POST /orders"],
        ),
    ];

    settle_duplicates(&mut features, &units);

    let summary: Vec<(&str, &[String])> = features
        .iter()
        .map(|f| (f.slug.as_str(), f.entry_points.as_slice()))
        .collect();
    assert_eq!(
        summary,
        [
            ("routes", &["orders.rb#GET /routes".to_string()][..]),
            (
                "orders",
                &[
                    "orders.rb#GET /orders".to_string(),
                    "orders.rb#POST /orders".to_string()
                ][..]
            ),
        ]
    );
    assert_eq!(features[0].source_paths, vec!["orders.rb"]);
}

#[tokio::test]
async fn short_tags_stand_for_the_entry_points_of_a_domain() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let only_orders = DomainMap {
        domains: vec![domains.domains[0].clone()],
    };
    let llm = FakeLlm::answering(
        r#"{"features":[{"slug":"a","name":"A","description":"d","entry_points":["E2","e9"]},
            {"slug":"b","name":"B","description":"d","entry_points":["E1"]}]}"#,
    );

    let features = run(dir.path(), &only_orders, &entries, &llm).await;

    // `E2` is the second entry point of the domain; `e9` does not exist.
    assert_eq!(features[0].entry_points, vec!["orders.rb#POST /orders"]);
    assert_eq!(features[1].entry_points, vec!["orders.rb#GET /orders"]);
}
