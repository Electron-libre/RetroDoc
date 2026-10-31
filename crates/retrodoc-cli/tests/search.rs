//! `retrodoc search` on the artifacts of a run: no LLM, the hits or "not documented".

use std::process::Command;

use retrodoc_core::config::Config;
use retrodoc_core::model::Feature;
use retrodoc_pipeline::{DomainCluster, DomainMap};

fn feature(slug: &str, name: &str, description: &str) -> Feature {
    Feature {
        back_office: false,
        slug: slug.into(),
        domain_slug: "delivery".into(),
        name: name.into(),
        description: description.into(),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: None,
    }
}

fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    Config::write_default(dir.path(), false).unwrap();
    let features = [
        feature(
            "assign-rider",
            "Assign a rider",
            "The closest rider takes the order",
        ),
        feature(
            "track-rider",
            "Track a rider",
            "The rider position on the map",
        ),
        feature(
            "plan-route",
            "Plan a route",
            "The rider gets the shortest path",
        ),
    ];
    retrodoc_pipeline::save_features(dir.path(), &features).unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "delivery".into(),
            name: "Delivery".into(),
            description: "Orders brought to customers".into(),
            paths: vec![],
            ..DomainCluster::default()
        }],
    }
    .save(dir.path())
    .unwrap();
    dir
}

fn search(repo: &tempfile::TempDir, query: &str, limit: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["search", query, "--limit", limit, "--path"])
        .arg(repo.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn titles(out: &str) -> Vec<&str> {
    out.lines()
        .filter(|l| l.starts_with('['))
        .map(|l| l.split_once("] ").unwrap().1.split(" (").next().unwrap())
        .collect()
}

// spec: DSR-1
#[test]
fn the_hits_come_best_first_up_to_the_limit() {
    let repo = generated_repo();
    assert_eq!(
        titles(&search(&repo, "plan route", "10"))[0],
        "Plan a route"
    );
    assert_eq!(titles(&search(&repo, "rider", "2")).len(), 2);
}

// spec: DSR-2, DSR-3, DSR-4
#[test]
fn a_query_whose_rare_word_no_entry_has_is_not_documented() {
    let repo = generated_repo();
    let out = search(&repo, "rider salary", "10");
    assert_eq!(titles(&out), Vec::<&str>::new(), "{out}");
    assert!(out.starts_with("not documented"), "{out}");
    assert!(
        out.contains("3 entries share only its common words"),
        "{out}"
    );
    assert!(out.contains("try other words"), "{out}");
}

// spec: DSR-3, DSR-4
#[test]
fn a_query_sharing_no_word_is_not_documented_without_a_count() {
    let repo = generated_repo();
    let out = search(&repo, "zzzz", "10");
    assert!(out.starts_with("not documented"), "{out}");
    assert!(!out.contains("share only"), "{out}");
}
