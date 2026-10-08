use super::*;

use std::path::PathBuf;

use retrodoc_ingest::{FileEntry, FileKind};

use crate::testing::FakeLlm;

fn repo(files: &[(&str, &str)]) -> (tempfile::TempDir, IngestResult) {
    let dir = tempfile::tempdir().unwrap();
    let mut entries = Vec::new();
    for (path, content) in files {
        let abs = dir.path().join(path);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(&abs, content).unwrap();
        entries.push(FileEntry {
            path: PathBuf::from(path),
            kind: FileKind::Other,
            size_bytes: content.len() as u64,
        });
    }
    let ingest = IngestResult {
        files: entries,
        history_by_path: std::collections::HashMap::new(),
        existing_docs: Vec::new(),
        commits: Vec::new(),
    };
    (dir, ingest)
}

/// A stack the sniffing knows nothing about: catalogs in a custom folder,
/// without a language in their path.
fn custom_stack() -> Vec<(&'static str, &'static str)> {
    vec![
        ("texts/shop.strings.yml", "cart:\n  title: Cart\n"),
        ("texts/billing.strings.yml", "invoice:\n  title: Invoice\n"),
        ("db/model.sql", "CREATE TABLE orders (id int);"),
    ]
}

fn rules_answer(rules: &[(&str, &str, &str)]) -> String {
    let rules: Vec<String> = rules
        .iter()
        .map(|(kind, glob, format)| {
            format!(r#"{{"kind":"{kind}","glob":"{glob}","format":"{format}"}}"#)
        })
        .collect();
    format!(r#"{{"rules":[{}]}}"#, rules.join(","))
}

#[tokio::test]
async fn infers_the_rules_checks_them_and_saves_them() {
    let (dir, ingest) = repo(&custom_stack());
    let llm = FakeLlm::answering(rules_answer(&[
        ("i18n", "texts/*.yml", "yaml"),
        ("schema", "db/*.sql", "sql_ddl"),
    ]));

    let map = infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();

    assert_eq!(llm.calls(), 1);
    assert_eq!(map.rules.len(), 2);
    assert_eq!(map.files(SourceKind::I18n, &ingest.files).len(), 2);
    let prompt = llm.prompts().remove(0);
    assert!(prompt.contains("texts/"), "{prompt}");
    assert!(prompt.contains("schema sql_ddl db/model.sql"), "{prompt}");
    let saved = SourceMapFile::load(dir.path()).unwrap();
    assert_eq!(saved.rules, map.rules);
}

#[tokio::test]
async fn a_current_file_is_reused_without_a_call() {
    let (dir, ingest) = repo(&custom_stack());
    let llm = FakeLlm::answering(rules_answer(&[("i18n", "texts/*.yml", "yaml")]));
    infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();

    // Another file of an existing shape does not make the rules out of date.
    let (_, mut bigger) = repo(&custom_stack());
    std::fs::write(dir.path().join("texts/more.strings.yml"), "a:\n  b: c\n").unwrap();
    bigger.files.push(FileEntry {
        path: PathBuf::from("texts/more.strings.yml"),
        kind: FileKind::Other,
        size_bytes: 1,
    });
    infer_sources(dir.path(), &bigger, &llm, false)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 1);

    // `force` asks again.
    infer_sources(dir.path(), &ingest, &llm, true)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn a_new_kind_of_folder_infers_again_unless_the_file_was_edited() {
    let (dir, ingest) = repo(&custom_stack());
    let llm = FakeLlm::answering(rules_answer(&[("i18n", "texts/*.yml", "yaml")]));
    infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();

    let mut changed = ingest.clone();
    changed.files.push(FileEntry {
        path: PathBuf::from("new_dir/x.yml"),
        kind: FileKind::Other,
        size_bytes: 1,
    });
    infer_sources(dir.path(), &changed, &llm, false)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);

    // A hand edit is kept, even if the tree changed again.
    let mut saved = SourceMapFile::load(dir.path()).unwrap();
    saved.rules = vec![SourceRule {
        kind: SourceKind::I18n,
        glob: "texts/billing.*".to_string(),
        format: SourceFormat::Yaml,
    }];
    saved.tree_hash = "old".to_string();
    saved.save(dir.path()).unwrap();
    let map = infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);
    assert_eq!(map.rules, saved.rules);
}

#[tokio::test]
async fn a_rule_that_reads_nothing_is_sent_back_once_and_the_fix_is_kept() {
    let (dir, ingest) = repo(&custom_stack());
    let llm = FakeLlm::sequence(&[
        &rules_answer(&[
            ("i18n", "wrong/*.yml", "yaml"),
            ("schema", "db/*.sql", "sql_ddl"),
        ]),
        &rules_answer(&[
            ("i18n", "texts/*.yml", "yaml"),
            ("schema", "db/*.sql", "sql_ddl"),
        ]),
    ]);

    let map = infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();

    assert_eq!(llm.calls(), 2);
    let globs: Vec<_> = map.rules.iter().map(|r| r.glob.as_str()).collect();
    assert_eq!(globs, ["db/*.sql", "texts/*.yml"]);
    let retry = llm.prompts().remove(1);
    assert!(
        retry.contains("i18n yaml wrong/*.yml — the glob selects no file"),
        "{retry}"
    );
}

#[tokio::test]
async fn a_rule_still_bad_after_the_retry_is_dropped_and_the_sniffed_source_stays() {
    let (dir, ingest) = repo(&custom_stack());
    // Only the SQL file is sniffed: the yml files have no language anywhere.
    let llm = FakeLlm::answering(rules_answer(&[
        ("schema", "texts/*.yml", "sql_ddl"),
        ("i18n", "nothing/*", "yaml"),
    ]));

    let map = infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();

    assert_eq!(llm.calls(), 2);
    assert_eq!(map.rules.len(), 1);
    assert_eq!(map.files(SourceKind::Schema, &ingest.files).len(), 1);
    // The answer was understood: it is saved, not asked again.
    infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn an_unreadable_saved_file_is_left_alone() {
    let (dir, ingest) = repo(&custom_stack());
    let path = Artifact::SignalSources.path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "rules: [oops").unwrap();
    let llm = FakeLlm::answering(rules_answer(&[("i18n", "texts/*.yml", "yaml")]));

    let map = infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();

    assert_eq!(llm.calls(), 0);
    assert_eq!(map, SourceMap::sniff(dir.path(), &ingest.files));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "rules: [oops");
}

#[tokio::test]
async fn the_sniffed_sources_the_llm_left_out_are_kept() {
    let (dir, ingest) = repo(&custom_stack());
    let llm = FakeLlm::answering(rules_answer(&[("i18n", "texts/*.yml", "yaml")]));

    let map = infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();

    assert_eq!(map.files(SourceKind::I18n, &ingest.files).len(), 2);
    assert_eq!(map.files(SourceKind::Schema, &ingest.files).len(), 1);
}

#[tokio::test]
async fn an_llm_failure_or_nonsense_leaves_the_sniffed_map() {
    let (dir, ingest) = repo(&custom_stack());
    let sniffed = SourceMap::sniff(dir.path(), &ingest.files);

    let failing = FakeLlm::failing_after(0, |_| String::new());
    let map = infer_sources(dir.path(), &ingest, &failing, false)
        .await
        .unwrap();
    assert_eq!(map, sniffed);

    let nonsense = FakeLlm::answering("no idea");
    let map = infer_sources(dir.path(), &ingest, &nonsense, false)
        .await
        .unwrap();
    assert_eq!(map, sniffed);
    assert!(SourceMapFile::load(dir.path()).is_none());
}

#[tokio::test]
async fn unknown_kinds_and_formats_are_ignored() {
    let (dir, ingest) = repo(&custom_stack());
    let llm = FakeLlm::answering(rules_answer(&[
        ("i18n", "texts/*.yml", "yaml"),
        ("i18n", "texts/*.yml", "xliff"),
        ("graphql", "db/*.sql", "sql_ddl"),
        ("i18n", "", "yaml"),
    ]));

    let map = infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();

    assert_eq!(llm.calls(), 1);
    assert_eq!(map.files(SourceKind::I18n, &ingest.files).len(), 2);
}

#[tokio::test]
async fn without_an_llm_the_saved_sources_or_else_the_sniffed_ones_are_used() {
    let (dir, ingest) = repo(&custom_stack());
    assert_eq!(
        saved_or_sniffed(dir.path(), &ingest.files),
        SourceMap::sniff(dir.path(), &ingest.files)
    );
    let llm = FakeLlm::answering(rules_answer(&[("i18n", "texts/*.yml", "yaml")]));
    let inferred = infer_sources(dir.path(), &ingest, &llm, false)
        .await
        .unwrap();
    assert_eq!(saved_or_sniffed(dir.path(), &ingest.files), inferred);
}
