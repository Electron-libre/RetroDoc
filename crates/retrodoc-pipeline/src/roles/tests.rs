use super::*;

use crate::testing::FakeLlm;

fn entry(path: &str, kind: FileKind) -> FileEntry {
    FileEntry {
        path: PathBuf::from(path),
        kind,
        size_bytes: 1,
    }
}

fn rules(list: &[(&str, FileRole)]) -> RoleRules {
    RoleRules {
        stack: "test".to_string(),
        rules: list
            .iter()
            .map(|(pattern, role)| RoleRule {
                pattern: (*pattern).to_string(),
                role: *role,
            })
            .collect(),
        ..RoleRules::default()
    }
}

#[test]
fn promotes_files_of_identified_languages() {
    let mut files = vec![
        entry("lib/a.ex", FileKind::Other),
        entry("app/show.html.erb", FileKind::Other),
    ];
    let rules = RoleRules {
        source_extensions: vec!["ex".to_string()],
        ..RoleRules::default()
    };
    assert_eq!(rules.promote_sources(&mut files), 1);
    assert_eq!(files[0].kind, FileKind::Source);
    assert_eq!(files[1].kind, FileKind::Other);
    // An older roles.yaml has no such key: nothing is promoted.
    let old: RoleRules = serde_yaml::from_str("stack: x\nrules: []\n").unwrap();
    assert!(old.source_extensions.is_empty());
}

#[test]
fn most_specific_rule_wins() {
    let rules = rules(&[
        ("app/**", FileRole::Logic),
        ("app/models/**", FileRole::Model),
        ("**/*.erb", FileRole::View),
        ("app/controllers/", FileRole::EntryPoint),
    ]);
    let map = rules.classify(&[
        entry("app/models/user.rb", FileKind::Source),
        entry("app/services/pay.rb", FileKind::Source),
        entry("app/models/show.html.erb", FileKind::Other),
        entry("app/controllers/a/b.rb", FileKind::Source),
        entry("lib/x.rb", FileKind::Source),
    ]);
    let role = |p: &str| map.roles[Path::new(p)];
    assert_eq!(role("app/models/user.rb"), FileRole::Model);
    assert_eq!(role("app/services/pay.rb"), FileRole::Logic);
    assert_eq!(role("app/models/show.html.erb"), FileRole::Model);
    assert_eq!(role("app/controllers/a/b.rb"), FileRole::EntryPoint);
    assert_eq!(role("lib/x.rb"), FileRole::Unclassified);
}

#[test]
fn tests_and_docs_are_fixed_and_bad_rules_skipped() {
    let rules = rules(&[("**", FileRole::Logic), ("a[", FileRole::Model)]);
    let map = rules.classify(&[
        entry("spec/a_spec.rb", FileKind::Test),
        entry("README.md", FileKind::Markdown),
        entry("a.rb", FileKind::Source),
    ]);
    assert_eq!(map.roles[Path::new("spec/a_spec.rb")], FileRole::Test);
    assert_eq!(map.roles[Path::new("README.md")], FileRole::Docs);
    assert_eq!(map.roles[Path::new("a.rb")], FileRole::Logic);
    let dist = map.distribution();
    assert_eq!(dist[&FileRole::Logic], 1);
    assert_eq!(map.files_with(FileRole::Test).len(), 1);
}

#[test]
fn entrypoint_role_round_trips() {
    let parsed: RolesResponse = serde_json::from_str(
        r#"{"rules":[{"pattern":"a","role":"entrypoint"},{"pattern":"b","role":"entry_point"}]}"#,
    )
    .unwrap();
    assert!(parsed.rules.iter().all(|r| r.role == FileRole::EntryPoint));
    assert!(serde_yaml::to_string(&parsed.rules[0].role)
        .unwrap()
        .contains("entrypoint"));
}

#[test]
fn unknown_role_name_parses_as_unclassified() {
    let parsed: RolesResponse =
        serde_json::from_str(r#"{"rules":[{"pattern":"x/**","role":"banana"}]}"#).unwrap();
    assert_eq!(parsed.rules[0].role, FileRole::Unclassified);
}

#[test]
fn tree_is_compact_and_bounded() {
    let files: Vec<FileEntry> = (0..400)
        .map(|i| entry(&format!("d{i}/sub/f.rb"), FileKind::Source))
        .chain((0..10).map(|i| entry(&format!("app/f{i}.rb"), FileKind::Source)))
        .collect();
    let tree = render_tree(&files);
    assert!(tree.contains("app/ — 10 file(s) [.rb×10] e.g. f0.rb, f1.rb, f2.rb, f3.rb"));
    assert!(tree.contains("deeper directories not shown"));
    assert!(tree.lines().count() <= MAX_TREE_DIRS + 1);
}

#[tokio::test]
async fn identifies_once_then_reuses_saved_rules() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    let ingest = IngestResult {
        files: vec![entry("src/lib.rs", FileKind::Source)],
        history_by_path: std::collections::HashMap::new(),
        existing_docs: Vec::new(),
    };
    let llm = FakeLlm::answering(
        r#"```json
{"stack":"Rust library","rules":[{"pattern":"src/**","role":"logic"}],
"chunk_boundaries":[{"extensions":["rs"],"pattern":"^\\s*(pub )?fn "}]}
```"#,
    );

    let first = identify_roles(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();
    assert_eq!(first.stack, "Rust library");
    let second = identify_roles(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();
    assert_eq!(second.rules.len(), 1);
    // The chunk boundaries survive the save and reload, and travel with the map.
    assert_eq!(second.chunk_boundaries.len(), 1);
    assert_eq!(second.chunk_boundaries[0].pattern, r"^\s*(pub )?fn ");
    let map = second.classify(&ingest.files);
    assert_eq!(map.chunk_boundaries, second.chunk_boundaries);
    assert_eq!(llm.calls(), 1);

    identify_roles(dir.path(), &ingest, &llm, true)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);
}
