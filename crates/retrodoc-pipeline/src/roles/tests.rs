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
    assert_eq!(old.source_extensions, Vec::<String>::new());
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
async fn parsed_languages_get_no_boundary_rule_and_the_prompt_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let ingest = IngestResult {
        files: vec![entry("src/lib.rs", FileKind::Source)],
        history_by_path: std::collections::HashMap::new(),
        existing_docs: Vec::new(),
        commits: Vec::new(),
    };
    let llm = FakeLlm::answering(
        r#"{"stack":"Rust","rules":[{"pattern":"src/**","role":"logic"}],
"chunk_boundaries":[{"extensions":["rs"],"pattern":"^\\s*fn "}]}"#,
    );

    let rules = identify_roles(dir.path(), &ingest, &ProductBrief::default(), &llm, false)
        .await
        .unwrap();

    assert_eq!(rules.chunk_boundaries, Vec::<ChunkBoundary>::new());
    assert_eq!(llm.calls(), 1, "no call to check or fix the rule");
    let system = &llm.system_prompts()[0];
    assert!(system.contains("Do not give `chunk_boundaries` for the extensions rb"));
}

#[tokio::test]
async fn identifies_once_then_reuses_saved_rules() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    let ingest = IngestResult {
        files: vec![entry("src/lib.zig", FileKind::Source)],
        history_by_path: std::collections::HashMap::new(),
        existing_docs: Vec::new(),
        commits: Vec::new(),
    };
    let llm = FakeLlm::answering(
        r#"```json
{"stack":"Zig library","rules":[{"pattern":"src/**","role":"logic"}],
"chunk_boundaries":[{"extensions":["zig"],"pattern":"^\\s*(pub )?fn "}]}
```"#,
    );

    let first = identify_roles(dir.path(), &ingest, &ProductBrief::default(), &llm, false)
        .await
        .unwrap();
    assert_eq!(first.stack, "Zig library");
    let second = identify_roles(dir.path(), &ingest, &ProductBrief::default(), &llm, false)
        .await
        .unwrap();
    assert_eq!(second.rules.len(), 1);
    // The chunk boundaries survive the save and reload, and travel with the map.
    assert_eq!(second.chunk_boundaries.len(), 1);
    assert_eq!(second.chunk_boundaries[0].pattern, r"^\s*(pub )?fn ");
    let map = second.classify(&ingest.files);
    assert_eq!(map.chunk_boundaries, second.chunk_boundaries);
    assert_eq!(llm.calls(), 1);

    identify_roles(dir.path(), &ingest, &ProductBrief::default(), &llm, true)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn the_brief_heads_the_prompt_but_does_not_redo_saved_rules() {
    let dir = tempfile::tempdir().unwrap();
    let ingest = IngestResult {
        files: vec![entry("src/lib.rs", FileKind::Source)],
        history_by_path: std::collections::HashMap::new(),
        existing_docs: Vec::new(),
        commits: Vec::new(),
    };
    let llm =
        FakeLlm::answering(r#"{"stack":"Rust","rules":[{"pattern":"src/**","role":"logic"}]}"#);
    let brief = crate::testing::brief("Sells things to buyers.");

    identify_roles(dir.path(), &ingest, &brief, &llm, false)
        .await
        .unwrap();
    let prompt = &llm.prompts()[0];
    assert!(
        prompt.starts_with("Product brief of the application"),
        "{prompt}"
    );
    assert!(prompt.find("Purpose:").unwrap() < prompt.find("File tree").unwrap());

    // A different brief does not make the saved rules out of date.
    let other = crate::testing::brief("Rents things to renters.");
    identify_roles(dir.path(), &ingest, &other, &llm, false)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 1);
}

#[test]
fn a_slice_of_the_role_map_keeps_its_files_and_every_test_file() {
    let mut map = RoleMap::default();
    for (path, role) in [
        ("a/controller.rb", FileRole::EntryPoint),
        ("b/controller.rb", FileRole::EntryPoint),
        ("a/model.rb", FileRole::Model),
        ("spec/a_spec.rb", FileRole::Test),
        ("spec/b_spec.rb", FileRole::Test),
    ] {
        map.roles.insert(PathBuf::from(path), role);
    }

    let slice = map.only(&[
        PathBuf::from("a/controller.rb"),
        PathBuf::from("a/model.rb"),
    ]);

    let kept: Vec<&str> = slice.roles.keys().map(|p| p.to_str().unwrap()).collect();
    assert_eq!(
        kept,
        [
            "a/controller.rb",
            "a/model.rb",
            "spec/a_spec.rb",
            "spec/b_spec.rb"
        ]
    );
    assert_eq!(
        slice.files_with(FileRole::EntryPoint),
        [Path::new("a/controller.rb")]
    );
}

#[test]
fn a_manifest_with_invalid_utf8_is_shown_replaced() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Gemfile"), b"# caf\xe9\ngem \"rails\"\n").unwrap();
    assert_eq!(
        manifest_excerpts(dir.path()),
        "\n--- Gemfile ---\n# caf\u{fffd}\ngem \"rails\"\n\n"
    );
}

#[cfg(unix)]
#[test]
fn a_manifest_linking_outside_the_repo_is_left_out() {
    let repo = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret"), repo.path().join("Gemfile")).unwrap();
    assert_eq!(manifest_excerpts(repo.path()), "");
}
