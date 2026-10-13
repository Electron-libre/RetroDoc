use super::*;

use crate::entry_points::{EntryFile, EntryKind, EntryPoint};
use crate::glossary::MergedEntity;

fn feature(entry_points: &[&str]) -> Feature {
    Feature {
        back_office: false,
        slug: "f".to_string(),
        domain_slug: "d".to_string(),
        sub_domain_slug: None,
        name: "F".to_string(),
        description: "d".to_string(),
        source_paths: Vec::new(),
        entry_points: entry_points.iter().map(ToString::to_string).collect(),
        confidence: None,
    }
}

fn entry(name: &str) -> EntryPoint {
    EntryPoint {
        kind: EntryKind::HttpRoute,
        name: name.to_string(),
        verb: "get".to_string(),
        resource: "r".to_string(),
        description: String::new(),
        outputs: Vec::new(),
    }
}

#[test]
fn files_reached_by_no_documented_behaviour_are_technical_support() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (name, content) in [
        ("routes.rb", "def a\n  Signer.call\nend\n"),
        ("signer.rb", "class Signer\nend\n"),
        ("contract.rb", "class Contract\nend\n"),
        ("helper.rb", "module Helper\nend\n"),
        ("other_routes.rb", "def b\nend\n"),
    ] {
        std::fs::write(root.join(name), content).unwrap();
    }
    let sources: Vec<PathBuf> = [
        "routes.rb",
        "signer.rb",
        "contract.rb",
        "helper.rb",
        "other_routes.rb",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    let entry_points = EntryPoints {
        files: [
            ("routes.rb", "GET /a"),
            // An entry point no feature holds: its file is not reached through it.
            ("other_routes.rb", "GET /b"),
        ]
        .iter()
        .map(|(file, name)| {
            (
                PathBuf::from(file),
                EntryFile {
                    content_hash: String::new(),
                    entry_points: vec![entry(name)],
                },
            )
        })
        .collect(),
    };
    let surface = Surface {
        entities: vec![MergedEntity {
            name: "Contract".to_string(),
            description: String::new(),
            attributes: Vec::new(),
            associations: Vec::new(),
            files: vec![PathBuf::from("contract.rb")],
        }],
        resources: Vec::new(),
    };
    // An unplaced list saved before is kept.
    BehaviourCoverage {
        unplaced_entry_points: vec!["other_routes.rb#GET /b".to_string()],
        technical_support: Vec::new(),
        by_domain: std::collections::BTreeMap::new(),
    }
    .save(root)
    .unwrap();

    let coverage = save_technical_support(
        root,
        &sources,
        &entry_points,
        &[feature(&["routes.rb#GET /a"])],
        &surface,
    )
    .unwrap();

    assert_eq!(
        coverage.technical_support,
        vec![PathBuf::from("helper.rb"), PathBuf::from("other_routes.rb")]
    );
    assert_eq!(
        coverage.unplaced_entry_points,
        vec!["other_routes.rb#GET /b"]
    );
    assert_eq!(BehaviourCoverage::load(root).unwrap(), coverage);
}

#[test]
fn a_coverage_saved_before_the_counts_per_domain_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let path = Artifact::BehaviourCoverage.path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "unplaced_entry_points:\n- a.rb#GET /a\ntechnical_support: []\n",
    )
    .unwrap();

    let coverage = BehaviourCoverage::load(dir.path()).unwrap();

    assert_eq!(coverage.unplaced_entry_points, ["a.rb#GET /a"]);
    assert!(coverage.by_domain.is_empty());
}
