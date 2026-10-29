use std::path::PathBuf;

use super::*;

use crate::artifact::{Artifact, Retention};
use crate::behaviour_coverage::{BehaviourCoverage, EntryCount};
use crate::domains::UNCATEGORIZED_SLUG;
use crate::entry_points::{EntryFile, EntryKind};
use crate::testing::{killed, FakeLlm};
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

/// `orders.rb` serves two resources of two domains; `GET /ping` is a resource of no domain.
fn fixture() -> (DomainMap, EntryPoints) {
    let domains = DomainMap {
        domains: vec![domain("orders", &["order"]), domain("routing", &["route"])],
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
        Retention::OnlyRead,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn groups_the_entry_points_of_each_domain_uncategorized_included() {
    let dir = tempfile::tempdir().unwrap();
    let (mut domains, entries) = fixture();
    domains.domains.push(domain(UNCATEGORIZED_SLUG, &["misc"]));
    let llm = FakeLlm::replying(|n, _| {
        Ok(match n {
            0 => {
                r#"{"features":[{"slug":"Place order","name":"Place order","description":"d",
                "entry_points":["orders.rb#POST /orders","orders.rb#GET /orders"]}]}"#
            }
            1 => {
                r#"{"features":[{"slug":"list-routes","name":"List routes","description":"d",
                "entry_points":["orders.rb#GET /routes"]}]}"#
            }
            _ => {
                r#"{"features":[{"slug":"check-health","name":"Check health","description":"d",
                "entry_points":["misc.rb#GET /ping"]}]}"#
            }
        }
        .to_string())
    });

    let features = run(dir.path(), &domains, &entries, &llm).await;

    assert_eq!(
        llm.calls(),
        3,
        "one call per domain, the uncategorized one too, and none to place: every entry point is held"
    );
    let coverage = BehaviourCoverage::load(dir.path()).unwrap();
    assert_eq!(coverage.unplaced_entry_points, Vec::<String>::new());
    assert_eq!(features.len(), 3);
    assert_eq!(features[0].slug, "place-order");
    assert_eq!(features[0].domain_slug, "orders");
    assert_eq!(
        features[0].entry_points,
        vec!["orders.rb#POST /orders", "orders.rb#GET /orders"]
    );
    assert_eq!(features[0].source_paths, vec!["orders.rb"]);
    assert_eq!(features[1].entry_points, vec!["orders.rb#GET /routes"]);
    assert_eq!(features[2].domain_slug, UNCATEGORIZED_SLUG);
    assert_eq!(features[2].entry_points, vec!["misc.rb#GET /ping"]);
    assert_eq!(load_features(dir.path()).unwrap().len(), 3);

    let again = run(dir.path(), &domains, &entries, &llm).await;
    assert_eq!(again.len(), 3);
    assert_eq!(llm.calls(), 3, "an unchanged rerun makes no call");
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
async fn the_back_office_flag_is_read_and_kept_by_a_rerun_and_a_partial_redo() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let answer = |n: usize, _: &CompletionRequest| -> Result<String, LlmError> {
        Ok(if n == 0 {
            r#"{"features":[{"slug":"orders","name":"O","description":"d","back_office":true,
                "entry_points":["orders.rb#GET /orders","orders.rb#POST /orders"]}]}"#
        } else {
            r#"{"features":[{"slug":"routes","name":"R","description":"d",
                "entry_points":["orders.rb#GET /routes"]}]}"#
        }
        .to_string())
    };
    let first = run(dir.path(), &domains, &entries, &FakeLlm::replying(answer)).await;
    assert!(first[0].back_office, "read from the answer");
    assert!(!first[1].back_office, "absent means a business feature");

    let again = FakeLlm::replying(answer);
    let reused = run(dir.path(), &domains, &entries, &again).await;
    assert_eq!(again.calls(), 0);
    assert!(reused[0].back_office, "kept when nothing is asked again");
    assert!(load_features(dir.path()).unwrap()[0].back_office);

    // The routing domain is asked again; the other one is reused as it was.
    let mut changed = entries.clone();
    changed
        .files
        .get_mut(Path::new("orders.rb"))
        .unwrap()
        .entry_points[2]
        .description = "now does something else".to_string();
    let redo = FakeLlm::replying(|_, _| {
        Ok(
            r#"{"features":[{"slug":"routes","name":"R","description":"d",
            "entry_points":["orders.rb#GET /routes"]}]}"#
                .to_string(),
        )
    });
    let redone = run(dir.path(), &domains, &changed, &redo).await;
    assert_eq!(redo.calls(), 1);
    assert!(
        redone
            .iter()
            .find(|f| f.slug == "orders")
            .unwrap()
            .back_office
    );
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
        back_office: false,
        slug: slug.to_string(),
        domain_slug: domain.to_string(),
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

// spec: RER-5
#[tokio::test]
async fn a_killed_run_keeps_the_domains_done_and_the_rerun_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let stuck = FakeLlm::hanging_after(1, |_| {
        r#"{"features":[{"slug":"orders","name":"O","description":"d",
            "entry_points":["orders.rb#GET /orders","orders.rb#POST /orders"]}]}"#
            .to_string()
    });
    killed(
        build_features_from_surface(
            dir.path(),
            &domains,
            &entries,
            &Surface::default(),
            &ProductBrief::default(),
            &Evidence::default(),
            &stuck,
            Retention::OnlyRead,
        ),
        &stuck,
    )
    .await;
    let saved = load_features(dir.path()).expect("the features of the domain done are on disk");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].domain_slug, "orders");

    let healthy = FakeLlm::answering(
        r#"{"features":[{"slug":"routes","name":"R","description":"d",
            "entry_points":["orders.rb#GET /routes"]}]}"#,
    );
    let features = run(dir.path(), &domains, &entries, &healthy).await;
    assert_eq!(features.len(), 2);
    assert_eq!(
        healthy.calls(),
        2,
        "the routing domain, then the placement of `GET /ping`: the orders domain is not asked again"
    );
}

#[tokio::test]
async fn a_killed_run_does_not_bring_back_the_features_of_the_removed_files_path() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let from_files = Feature {
        slug: "billing".to_string(),
        domain_slug: "billing".to_string(),
        name: "Billing".to_string(),
        description: "d".to_string(),
        source_paths: vec!["lib/billing.rb".to_string()],
        entry_points: Vec::new(),
        back_office: false,
        confidence: None,
    };
    save_features(dir.path(), &[from_files]).unwrap();
    let stuck = FakeLlm::hanging_after(1, |_| {
        r#"{"features":[{"slug":"orders","name":"O","description":"d",
            "entry_points":["orders.rb#GET /orders","orders.rb#POST /orders"]}]}"#
            .to_string()
    });
    killed(
        build_features_from_surface(
            dir.path(),
            &domains,
            &entries,
            &Surface::default(),
            &ProductBrief::default(),
            &Evidence::default(),
            &stuck,
            Retention::OnlyRead,
        ),
        &stuck,
    )
    .await;
    let saved = load_features(dir.path()).expect("the features of the domain done are on disk");
    let slugs: Vec<&str> = saved.iter().map(|f| f.slug.as_str()).collect();
    assert_eq!(slugs, vec!["orders"]);
}

/// Two domains of a context map: no resources, the entry points are those of their files.
fn contexts_fixture() -> (DomainMap, EntryPoints) {
    let context = |slug: &str, path: &str| DomainCluster {
        slug: slug.to_string(),
        name: slug.to_string(),
        description: "d".to_string(),
        paths: vec![PathBuf::from(path)],
        by_paths: true,
        ..DomainCluster::default()
    };
    let entries = entry_points(&[
        // Both files serve the same resource: the files tell the contexts apart, not the resource.
        (
            "orders.rb",
            vec![
                entry("GET /orders", "order"),
                entry("POST /orders", "order"),
            ],
        ),
        ("routes.rb", vec![entry("GET /routes", "order")]),
    ]);
    (
        DomainMap {
            domains: vec![
                context("orders", "orders.rb"),
                context("routing", "routes.rb"),
            ],
        },
        entries,
    )
}

fn only(domains: &DomainMap, slug: &str) -> DomainMap {
    DomainMap {
        domains: domains
            .domains
            .iter()
            .filter(|d| d.slug == slug)
            .cloned()
            .collect(),
    }
}

async fn run_keeping(
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
        Retention::Others,
    )
    .await
    .unwrap()
}

fn answer_by_domain() -> FakeLlm {
    FakeLlm::replying(|_, request| {
        Ok(if request.messages[1].content.contains("GET /routes") {
            r#"{"features":[{"slug":"routes","name":"R","description":"d",
                "entry_points":["routes.rb#GET /routes"]}]}"#
        } else {
            r#"{"features":[{"slug":"orders","name":"O","description":"d",
                "entry_points":["orders.rb#GET /orders","orders.rb#POST /orders"]}]}"#
        }
        .to_string())
    })
}

#[tokio::test]
async fn a_domain_without_resources_takes_the_entry_points_of_its_files_only() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = contexts_fixture();
    let llm = answer_by_domain();

    let features = run_keeping(dir.path(), &only(&domains, "orders"), &entries, &llm).await;

    assert_eq!(llm.calls(), 1);
    assert_eq!(features.len(), 1);
    assert_eq!(
        features[0].entry_points.len(),
        2,
        "not the route of the other file"
    );
}

#[tokio::test]
async fn running_one_domain_after_the_other_keeps_the_first_and_never_asks_for_it_again() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = contexts_fixture();
    let llm = answer_by_domain();

    run_keeping(dir.path(), &only(&domains, "orders"), &entries, &llm).await;
    let before = std::fs::read_to_string(Artifact::Features.path(dir.path())).unwrap();
    let both = run_keeping(dir.path(), &only(&domains, "routing"), &entries, &llm).await;

    assert_eq!(llm.calls(), 2, "one call per domain");
    let slugs: Vec<&str> = both.iter().map(|f| f.domain_slug.as_str()).collect();
    assert_eq!(slugs, ["orders", "routing"]);
    let saved = load_features(dir.path()).unwrap();
    assert_eq!(saved.len(), 2);
    assert!(
        before.contains("orders")
            && std::fs::read_to_string(Artifact::Features.path(dir.path()))
                .unwrap()
                .contains("slug: orders"),
        "the first feature is still there"
    );

    // Both domains together: nothing is asked, the first one's fingerprint was kept.
    let again = run_keeping(dir.path(), &domains, &entries, &llm).await;
    assert_eq!(llm.calls(), 2);
    assert_eq!(again.len(), 2);
}

#[tokio::test]
async fn without_retention_the_domains_not_given_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = contexts_fixture();
    let llm = answer_by_domain();
    run_keeping(dir.path(), &only(&domains, "orders"), &entries, &llm).await;

    let features = run(dir.path(), &only(&domains, "routing"), &entries, &llm).await;

    let slugs: Vec<&str> = features.iter().map(|f| f.domain_slug.as_str()).collect();
    assert_eq!(slugs, ["routing"]);
}

/// The orders domain claims one of its two entry points and the placement places nothing.
fn leaves_one_entry_point_unplaced() -> FakeLlm {
    FakeLlm::replying(|_, request| {
        let prompt = &request.messages[1].content;
        Ok(if prompt.contains("Entry points to place") {
            r#"{"placements":[]}"#
        } else if prompt.contains("GET /routes") {
            r#"{"features":[{"slug":"routes","name":"R","description":"d",
                "entry_points":["routes.rb#GET /routes"]}]}"#
        } else {
            r#"{"features":[{"slug":"orders","name":"O","description":"d",
                "entry_points":["orders.rb#GET /orders"]}]}"#
        }
        .to_string())
    })
}

#[tokio::test]
async fn the_entry_points_an_earlier_slice_left_unplaced_stay_listed() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = contexts_fixture();
    let llm = leaves_one_entry_point_unplaced();

    run_keeping(dir.path(), &only(&domains, "orders"), &entries, &llm).await;
    run_keeping(dir.path(), &only(&domains, "routing"), &entries, &llm).await;

    assert_eq!(
        BehaviourCoverage::load(dir.path())
            .unwrap()
            .unplaced_entry_points,
        ["orders.rb#POST /orders"]
    );
}

#[tokio::test]
async fn the_domains_run_together_after_each_alone_ask_nothing_again() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = contexts_fixture();
    let llm = leaves_one_entry_point_unplaced();
    run_keeping(dir.path(), &only(&domains, "orders"), &entries, &llm).await;
    run_keeping(dir.path(), &only(&domains, "routing"), &entries, &llm).await;
    let calls = llm.calls();
    assert_eq!(
        calls, 3,
        "the features of each, and the placement for the orders"
    );

    run_keeping(dir.path(), &domains, &entries, &llm).await;

    assert_eq!(llm.calls(), calls, "the placement is kept per domain");
    assert_eq!(
        BehaviourCoverage::load(dir.path())
            .unwrap()
            .unplaced_entry_points,
        ["orders.rb#POST /orders"]
    );
}

#[tokio::test]
async fn an_entry_point_is_only_placed_in_a_feature_of_its_own_domain() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = contexts_fixture();
    let llm = FakeLlm::replying(|_, request| {
        let prompt = &request.messages[1].content;
        Ok(if prompt.contains("Entry points to place") {
            // The prompt of a domain lists its own features only.
            assert!(!prompt.contains("- routes"), "{prompt}");
            r#"{"placements":[{"entry_point":"E1","feature":"orders"}]}"#
        } else if prompt.contains("GET /routes") {
            r#"{"features":[{"slug":"routes","name":"R","description":"d",
                "entry_points":["routes.rb#GET /routes"]}]}"#
        } else {
            r#"{"features":[{"slug":"orders","name":"O","description":"d",
                "entry_points":["orders.rb#GET /orders"]}]}"#
        }
        .to_string())
    });

    let features = run_keeping(dir.path(), &domains, &entries, &llm).await;

    let orders = features.iter().find(|f| f.slug == "orders").unwrap();
    assert_eq!(orders.entry_points.len(), 2, "the POST was placed");
    let routes = features.iter().find(|f| f.slug == "routes").unwrap();
    assert_eq!(routes.entry_points.len(), 1);
}

#[tokio::test]
async fn a_route_declared_in_another_file_is_documented_by_the_context_that_owns_it() {
    let dir = tempfile::tempdir().unwrap();
    let routed = "routes.rb#GET /routes";
    let context = |slug: &str, path: &str| DomainCluster {
        slug: slug.to_string(),
        name: slug.to_string(),
        description: "d".to_string(),
        paths: vec![PathBuf::from(path)],
        by_paths: true,
        ..DomainCluster::default()
    };
    let mut orders = context("orders", "orders.rb");
    orders.routed_in.insert(routed.to_string());
    let mut routing = context("routing", "routes.rb");
    routing.routed_out.insert(routed.to_string());
    let domains = DomainMap {
        domains: vec![orders, routing],
    };
    let entries = entry_points(&[
        ("orders.rb", vec![entry("GET /orders", "order")]),
        (
            "routes.rb",
            vec![entry("GET /routes", "order"), entry("GET /ping", "ping")],
        ),
    ]);
    let llm = FakeLlm::answering(
        r#"{"features":[{"slug":"all","name":"All","description":"d","entry_points":["E1","E2"]}]}"#,
    );

    let features = run(dir.path(), &domains, &entries, &llm).await;

    let ids = |domain: &str| -> Vec<String> {
        features
            .iter()
            .filter(|f| f.domain_slug == domain)
            .flat_map(|f| f.entry_points.clone())
            .collect()
    };
    assert_eq!(ids("orders"), ["orders.rb#GET /orders", routed]);
    assert_eq!(ids("routing"), ["routes.rb#GET /ping"]);
}

#[tokio::test]
async fn a_route_another_context_serves_stays_listed_when_the_file_that_declares_it_is_run() {
    let dir = tempfile::tempdir().unwrap();
    let (mut domains, entries) = contexts_fixture();
    let served = "routes.rb#GET /routes".to_string();
    domains.domains[0].routed_in.insert(served.clone());
    domains.domains[1].routed_out.insert(served.clone());
    // An earlier run of `orders` left the route unplaced.
    let earlier = BehaviourCoverage {
        unplaced_entry_points: vec![served.clone()],
        ..BehaviourCoverage::default()
    };
    earlier.save(dir.path()).unwrap();
    let llm = FakeLlm::answering(
        r#"{"features":[{"slug":"all","name":"A","description":"d","entry_points":["E1"]}]}"#,
    );

    run_keeping(dir.path(), &only(&domains, "routing"), &entries, &llm).await;

    assert_eq!(
        BehaviourCoverage::load(dir.path())
            .unwrap()
            .unplaced_entry_points,
        [served],
        "`routing` does not hold it: only a run of `orders` places it again"
    );
}

#[tokio::test]
async fn a_route_that_changes_context_redoes_the_features_of_the_one_that_gains_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut domains, entries) = contexts_fixture();
    let llm = FakeLlm::answering(
        r#"{"features":[{"slug":"all","name":"A","description":"d","entry_points":["E1"]}]}"#,
    );
    run(dir.path(), &domains, &entries, &llm).await;
    let calls = llm.calls();
    run(dir.path(), &domains, &entries, &llm).await;
    assert_eq!(llm.calls(), calls, "nothing changed: nothing asked");

    // A context file now matches the route: it moves from `routing` to `orders`.
    let served = "routes.rb#GET /routes".to_string();
    domains.domains[0].routed_in.insert(served.clone());
    domains.domains[1].routed_out.insert(served);
    run(dir.path(), &domains, &entries, &llm).await;

    assert!(
        llm.calls() > calls,
        "the features of `orders`, which gains an entry point, are asked again"
    );
}

/// A domain of two files, 100 and 60 entry points: over the limit of one prompt, so two slices.
fn big_domain() -> (DomainMap, EntryPoints) {
    let routes = |prefix: &str, n: usize| -> Vec<EntryPoint> {
        (0..n)
            .map(|i| entry(&format!("GET /{prefix}{i}"), prefix))
            .collect()
    };
    let domain = DomainCluster {
        slug: "big".to_string(),
        name: "Big".to_string(),
        description: "d".to_string(),
        paths: vec![PathBuf::from("a.rb"), PathBuf::from("b.rb")],
        by_paths: true,
        ..DomainCluster::default()
    };
    (
        DomainMap {
            domains: vec![domain],
        },
        entry_points(&[("a.rb", routes("a", 100)), ("b.rb", routes("b", 60))]),
    )
}

/// Each slice claims its first entry point in the feature `all`; the second one also in `later`.
fn answers_per_slice(prompt: &str) -> String {
    if prompt.contains("Entry points to place") {
        return r#"{"placements":[]}"#.to_string();
    }
    let extra = if prompt.contains("already found") {
        r#",{"slug":"later","name":"Later","description":"d","entry_points":["E2"]}"#
    } else {
        ""
    };
    format!(
        r#"{{"features":[{{"slug":"all","name":"All","description":"d{}","entry_points":["E1"]}}{extra}]}}"#,
        u8::from(prompt.contains("already found"))
    )
}

#[tokio::test]
async fn a_domain_over_the_prompt_limit_is_asked_in_slices_by_file_and_the_features_merge() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = big_domain();
    let llm = FakeLlm::replying(|_, request| Ok(answers_per_slice(&request.messages[1].content)));

    let features = run(dir.path(), &domains, &entries, &llm).await;

    let prompts = llm.prompts();
    assert_eq!(
        prompts.len(),
        5,
        "two slices, then the placement in 3 batches of 60"
    );
    assert!(
        prompts[0].contains("GET /a99") && !prompts[0].contains("/b0\""),
        "{}",
        prompts[0]
    );
    assert!(prompts[1].contains("GET /b59") && !prompts[1].contains("GET /a0\""));
    assert!(prompts[1].contains("already found") && prompts[1].contains("all"));
    assert!(!prompts[0].contains("already found"));
    assert!(!prompts[0].contains("(b.rb)") && !prompts[1].contains("(a.rb)"));
    let slugs: Vec<&str> = features.iter().map(|f| f.slug.as_str()).collect();
    assert_eq!(
        slugs,
        ["all", "later"],
        "`all` of the two slices is one feature"
    );
    assert_eq!(
        features[0].entry_points,
        ["a.rb#GET /a0", "b.rb#GET /b0"],
        "the entry points of the two slices"
    );
    assert_eq!(features[0].source_paths, ["a.rb", "b.rb"]);
    assert_eq!(
        features[0].description, "d0",
        "the first description is kept"
    );
}

#[tokio::test]
async fn a_run_killed_between_two_slices_keeps_the_first_and_the_rerun_asks_the_second() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = big_domain();
    let stuck = FakeLlm::hanging_after(1, |_| {
        r#"{"features":[{"slug":"all","name":"All","description":"d","entry_points":["E1"]}]}"#
            .to_string()
    });
    killed(
        build_features_from_surface(
            dir.path(),
            &domains,
            &entries,
            &Surface::default(),
            &ProductBrief::default(),
            &Evidence::default(),
            &stuck,
            Retention::OnlyRead,
        ),
        &stuck,
    )
    .await;
    let saved = load_features(dir.path()).expect("the first slice is on disk");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].entry_points, ["a.rb#GET /a0"]);

    let llm = FakeLlm::replying(|_, request| Ok(answers_per_slice(&request.messages[1].content)));
    let features = run(dir.path(), &domains, &entries, &llm).await;

    let prompts = llm.prompts();
    assert_eq!(
        prompts.len(),
        4,
        "the second slice, then the placement in 3 batches"
    );
    assert!(prompts[0].contains("GET /b59") && !prompts[0].contains("GET /a0\""));
    assert_eq!(features[0].entry_points, ["a.rb#GET /a0", "b.rb#GET /b0"]);
}

#[tokio::test]
async fn a_rerun_of_a_sliced_domain_asks_nothing_and_a_new_entry_point_redoes_its_slice_only() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, mut entries) = big_domain();
    let llm = FakeLlm::replying(|_, request| Ok(answers_per_slice(&request.messages[1].content)));
    let first = run(dir.path(), &domains, &entries, &llm).await;
    let calls = llm.calls();

    let again = run(dir.path(), &domains, &entries, &llm).await;
    assert_eq!(llm.calls(), calls, "nothing changed: nothing asked");
    assert_eq!(again.len(), first.len());

    entries
        .files
        .get_mut(&PathBuf::from("b.rb"))
        .unwrap()
        .entry_points
        .push(entry("GET /b60", "b"));
    run(dir.path(), &domains, &entries, &llm).await;
    let prompts = llm.prompts();
    let new = &prompts[calls..];
    assert!(
        new.iter().any(|p| p.contains("GET /b60")) && new.iter().all(|p| !p.contains("GET /a0\"")),
        "only the slice of `b.rb` is asked again"
    );
}

#[test]
fn slices_hold_whole_files_and_a_file_over_the_limit_is_cut() {
    let many = |n: usize| -> Vec<EntryPoint> {
        (0..n).map(|i| entry(&format!("GET /{i}"), "r")).collect()
    };
    let entries = entry_points(&[
        ("a.rb", many(100)),
        ("b.rb", many(40)),
        ("c.rb", many(20)),
        ("d.rb", many(400)),
        ("e.rb", many(10)),
    ]);
    let domain = DomainCluster {
        paths: ["a.rb", "b.rb", "c.rb", "d.rb", "e.rb"]
            .map(PathBuf::from)
            .to_vec(),
        by_paths: true,
        ..DomainCluster::default()
    };
    let held = domain_entries(&domain, &entries);

    let ranges = slices(&held);

    // a + b = 140 (c would make 160), c alone (d does not fit), d cut in 150, 150 and 100, then e.
    assert_eq!(
        ranges,
        vec![0..140, 140..160, 160..310, 310..460, 460..560, 560..570]
    );
    assert!(ranges.iter().all(|r| r.len() <= MAX_ENTRIES_PER_PROMPT));
    assert_eq!(
        ranges.iter().map(ExactSizeIterator::len).sum::<usize>(),
        570
    );
}

#[tokio::test]
async fn a_stop_in_the_first_slice_keeps_the_features_the_other_slices_had() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, mut entries) = big_domain();
    let llm = FakeLlm::replying(|_, request| Ok(answers_per_slice(&request.messages[1].content)));
    run(dir.path(), &domains, &entries, &llm).await;
    let before = load_features(dir.path()).unwrap();
    let in_b = |features: &[Feature]| {
        features
            .iter()
            .flat_map(|f| &f.entry_points)
            .filter(|id| id.starts_with("b.rb"))
            .count()
    };
    assert!(in_b(&before) > 0);

    // A new file before the others changes both slices; the run stops during the second.
    entries.files.insert(
        PathBuf::from("0.rb"),
        EntryFile {
            content_hash: "h".to_string(),
            entry_points: vec![entry("GET /zero", "zero")],
        },
    );
    let mut domains = domains;
    domains.domains[0].paths.push(PathBuf::from("0.rb"));
    // The first slice answers, the second one never does.
    let stuck = FakeLlm::hanging_after(1, |_| {
        r#"{"features":[{"slug":"all","name":"All","description":"d","entry_points":["E1"]}]}"#
            .to_string()
    });
    killed(
        build_features_from_surface(
            dir.path(),
            &domains,
            &entries,
            &Surface::default(),
            &ProductBrief::default(),
            &Evidence::default(),
            &stuck,
            Retention::OnlyRead,
        ),
        &stuck,
    )
    .await;

    let after = load_features(dir.path()).unwrap();
    assert_eq!(
        in_b(&after),
        in_b(&before),
        "the slice not asked yet is kept"
    );
}

#[tokio::test]
async fn a_failed_slice_keeps_the_features_it_had() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, mut entries) = big_domain();
    let llm = FakeLlm::replying(|_, request| Ok(answers_per_slice(&request.messages[1].content)));
    run(dir.path(), &domains, &entries, &llm).await;
    let before = load_features(dir.path()).unwrap();

    // A new entry point in the second file: that slice is asked again, and fails.
    entries
        .files
        .get_mut(&PathBuf::from("b.rb"))
        .unwrap()
        .entry_points
        .push(entry("GET /b60", "b"));
    let failing = FakeLlm::replying(|_, _| Err(LlmError::Transport("down".to_string())));
    let result = build_features_from_surface(
        dir.path(),
        &domains,
        &entries,
        &Surface::default(),
        &ProductBrief::default(),
        &Evidence::default(),
        &failing,
        Retention::OnlyRead,
    )
    .await;

    assert!(result.is_err());
    let after = load_features(dir.path()).unwrap();
    let ids = |features: &[Feature]| -> BTreeSet<String> {
        features
            .iter()
            .flat_map(|f| f.entry_points.clone())
            .collect()
    };
    assert_eq!(ids(&after), ids(&before), "nothing paid for is lost");
}

#[test]
fn features_merge_against_the_earlier_slices_only() {
    let feature = |slug: &str, name: &str, id: &str| Feature {
        back_office: false,
        slug: slug.to_string(),
        domain_slug: "big".to_string(),
        name: name.to_string(),
        description: format!("of {id}"),
        source_paths: vec![id.split('#').next().unwrap().to_string()],
        entry_points: vec![id.to_string()],
        confidence: None,
    };
    let mut features = Vec::new();

    // One answer: two features of the same name stay two.
    merge_features(
        &mut features,
        vec![
            feature("a", "Same", "a.rb#1"),
            feature("b", "Same", "a.rb#2"),
        ],
    );
    assert_eq!(features.len(), 2);

    // The next slice: by slug, or by name (the first hit), entry points and files united.
    merge_features(
        &mut features,
        vec![
            feature("a", "Other", "b.rb#3"),
            feature("c", "same", "b.rb#4"),
        ],
    );
    assert_eq!(features.len(), 2, "{features:?}");
    assert_eq!(features[0].entry_points, ["a.rb#1", "b.rb#3", "b.rb#4"]);
    assert_eq!(features[0].source_paths, ["a.rb", "b.rb"]);
    assert_eq!(features[0].description, "of a.rb#1");
}

#[tokio::test]
async fn the_entry_points_held_by_no_feature_are_counted_per_domain() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = contexts_fixture();
    let llm = leaves_one_entry_point_unplaced();

    run_keeping(dir.path(), &only(&domains, "orders"), &entries, &llm).await;
    run_keeping(dir.path(), &only(&domains, "routing"), &entries, &llm).await;

    let count = |entry_points, unplaced| EntryCount {
        entry_points,
        unplaced,
    };
    assert_eq!(
        BehaviourCoverage::load(dir.path()).unwrap().by_domain,
        BTreeMap::from([
            ("orders".to_string(), count(2, 1)),
            ("routing".to_string(), count(1, 0)),
        ]),
        "the second run keeps the count of the first"
    );
}

#[tokio::test]
async fn an_entry_point_placed_from_another_file_stays_in_its_feature_on_the_rerun() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    // The placement puts `GET /ping`, which belongs to no domain of the fixture (uncategorized), and
    // `POST /orders` into the orders feature.
    let answer = |n: usize, request: &CompletionRequest| -> Result<String, LlmError> {
        let prompt = &request.messages.last().unwrap().content;
        Ok(if prompt.contains("Entry points to place") {
            r#"{"placements":[{"entry_point":"orders.rb#POST /orders","feature":"orders"},
                {"entry_point":"misc.rb#GET /ping","feature":"orders"}]}"#
        } else if n == 0 {
            r#"{"features":[{"slug":"orders","name":"O","description":"d",
                "entry_points":["orders.rb#GET /orders"]}]}"#
        } else {
            r#"{"features":[{"slug":"routes","name":"R","description":"d",
                "entry_points":["orders.rb#GET /routes"]}]}"#
        }
        .to_string())
    };
    let first = run(dir.path(), &domains, &entries, &FakeLlm::replying(answer)).await;
    assert_eq!(first[0].entry_points.len(), 3);

    let again = FakeLlm::replying(answer);
    let features = run(dir.path(), &domains, &entries, &again).await;

    assert_eq!(again.calls(), 0, "nothing is asked again");
    assert_eq!(
        features[0].entry_points.len(),
        3,
        "the placed entry points are kept"
    );
}

/// The saved prints must not change with a refactor: another print redoes every user's next run.
#[tokio::test]
async fn the_print_of_a_domain_stays_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let llm = FakeLlm::answering(
        r#"{"features":[{"slug":"orders","name":"O","description":"d",
            "entry_points":["orders.rb#GET /orders"]}]}"#,
    );
    build_features_from_surface(
        dir.path(),
        &domains,
        &entries,
        &Surface::default(),
        &crate::testing::brief("Sells things."),
        &Evidence::default(),
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();
    let prints = crate::fingerprints::Fingerprints::load(dir.path()).features;
    assert_eq!(
        prints.into_iter().collect::<Vec<_>>(),
        vec![(
            "orders/-".to_string(),
            "750ccd02b35e6685334cdaa7e7e8eee2884f60833854b7054e6c0cb6058e9d4c".to_string()
        )]
    );
}
