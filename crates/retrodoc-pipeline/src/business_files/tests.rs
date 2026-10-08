use super::*;

use retrodoc_ingest::git_history::FileHistory;
use retrodoc_ingest::ExistingDoc;

use crate::testing::{brief, FakeLlm};

fn file(path: &str, kind: FileKind) -> FileEntry {
    FileEntry {
        path: PathBuf::from(path),
        kind,
        size_bytes: 10,
    }
}

fn repo(files: &[(&str, FileKind, &str)]) -> (tempfile::TempDir, IngestResult) {
    let dir = tempfile::tempdir().unwrap();
    let mut entries = Vec::new();
    for (path, kind, content) in files {
        let abs = dir.path().join(path);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(&abs, content).unwrap();
        entries.push(file(path, *kind));
    }
    let ingest = IngestResult {
        files: entries,
        history_by_path: std::collections::HashMap::new(),
        existing_docs: Vec::new(),
        commits: Vec::new(),
    };
    (dir, ingest)
}

/// A small library: no `models` folder, the business sits in plain classes.
fn library() -> (tempfile::TempDir, IngestResult) {
    repo(&[
        ("lib/shop/order.rb", FileKind::Source, "class Order; end"),
        (
            "lib/shop/pricing/discount.rb",
            FileKind::Source,
            "class Discount; end",
        ),
        (
            "lib/shop/pricing/tax.rb",
            FileKind::Source,
            "class Tax; end",
        ),
        (
            "lib/shop/http_client.rb",
            FileKind::Source,
            "class HttpClient; end",
        ),
        (
            "spec/order_spec.rb",
            FileKind::Test,
            "it 'totals the lines of an order' do\nend\n",
        ),
    ])
}

fn answer(entries: &[(&str, &str)]) -> String {
    let items: Vec<String> = entries
        .iter()
        .map(|(path, reason)| format!(r#"{{"path":"{path}","reason":"{reason}"}}"#))
        .collect();
    format!(r#"{{"business":[{}]}}"#, items.join(","))
}

async fn infer(
    dir: &tempfile::TempDir,
    ingest: &IngestResult,
    llm: &FakeLlm,
    force: bool,
) -> BusinessMap {
    infer_business_files(
        dir.path(),
        ingest,
        "A Ruby library",
        &ProductBrief::default(),
        llm,
        force,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn asks_once_keeps_the_ranked_entries_and_saves_them() {
    let (dir, ingest) = library();
    let llm = FakeLlm::answering(answer(&[
        ("lib/shop/order.rb", "an order"),
        ("lib/shop/pricing/", "discount rules"),
    ]));

    let map = infer(&dir, &ingest, &llm, false).await;

    assert_eq!(llm.calls(), 1);
    let paths: Vec<_> = map.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["lib/shop/order.rb", "lib/shop/pricing/"]);
    assert_eq!(map.entries[1].reason, "discount rules");
    assert_eq!(BusinessMap::load(dir.path()).unwrap(), map);
    assert_eq!(
        map.files(&ingest.files),
        [
            PathBuf::from("lib/shop/order.rb"),
            PathBuf::from("lib/shop/pricing/discount.rb"),
            PathBuf::from("lib/shop/pricing/tax.rb"),
        ]
    );
}

#[tokio::test]
async fn the_prompt_carries_the_stack_the_brief_and_the_evidence() {
    let (dir, mut ingest) = library();
    ingest.history_by_path.insert(
        PathBuf::from("lib/shop/order.rb"),
        FileHistory {
            commit_count: 7,
            ..FileHistory::default()
        },
    );
    ingest.existing_docs.push(ExistingDoc {
        path: PathBuf::from("README.md"),
        content: "# Shop\n\n## Pricing rules\ntext".to_string(),
    });
    let llm = FakeLlm::answering(answer(&[("lib/shop/order.rb", "an order")]));

    infer_business_files(
        dir.path(),
        &ingest,
        "A Ruby library",
        &brief("Sells meals"),
        &llm,
        false,
    )
    .await
    .unwrap();

    let prompt = llm.prompts().remove(0);
    assert!(prompt.contains("Sells meals"), "{prompt}");
    assert!(prompt.contains("Stack: A Ruby library"), "{prompt}");
    assert!(prompt.contains("order.rb (7)"), "{prompt}");
    assert!(prompt.contains("README.md: Pricing rules"), "{prompt}");
    assert!(prompt.contains("totals the lines of an order"), "{prompt}");
}

#[tokio::test]
async fn paths_that_do_not_exist_or_are_not_source_are_dropped() {
    let (dir, ingest) = library();
    let llm = FakeLlm::answering(answer(&[
        ("lib/shop/invented.rb", "made up"),
        ("spec/order_spec.rb", "a test"),
        ("./lib/shop/order.rb", "an order"),
        ("lib/shop/order.rb", "again"),
        ("lib/shop/pricing", "a folder without its slash"),
        ("", "empty"),
    ]));

    let map = infer(&dir, &ingest, &llm, false).await;

    let paths: Vec<_> = map.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["lib/shop/order.rb", "lib/shop/pricing/"]);
}

#[tokio::test]
async fn nothing_usable_gives_an_empty_map_that_is_not_saved() {
    let (dir, ingest) = library();
    let llm = FakeLlm::answering(answer(&[("nowhere/x.rb", "made up")]));

    let map = infer(&dir, &ingest, &llm, false).await;

    assert!(map.is_empty());
    assert!(BusinessMap::load(dir.path()).is_none());
    // Asked again next time.
    infer(&dir, &ingest, &llm, false).await;
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn a_failing_llm_gives_an_empty_map() {
    let (dir, ingest) = library();
    let llm = FakeLlm::failing_after(0, |_| String::new());

    let map = infer(&dir, &ingest, &llm, false).await;

    assert!(map.is_empty());
    assert!(BusinessMap::load(dir.path()).is_none());
}

#[tokio::test]
async fn a_current_file_is_reused_without_a_call() {
    let (dir, ingest) = library();
    let llm = FakeLlm::answering(answer(&[("lib/shop/order.rb", "an order")]));
    let first = infer(&dir, &ingest, &llm, false).await;

    let second = infer(&dir, &ingest, &llm, false).await;

    assert_eq!(llm.calls(), 1);
    assert_eq!(second, first);
}

#[tokio::test]
async fn a_new_file_in_a_known_folder_does_not_redo_it_but_a_new_folder_does() {
    let (dir, mut ingest) = library();
    let llm = FakeLlm::answering(answer(&[("lib/shop/order.rb", "an order")]));
    infer(&dir, &ingest, &llm, false).await;

    ingest
        .files
        .push(file("lib/shop/line.rb", FileKind::Source));
    infer(&dir, &ingest, &llm, false).await;
    assert_eq!(llm.calls(), 1);

    ingest
        .files
        .push(file("lib/billing/invoice.rb", FileKind::Source));
    infer(&dir, &ingest, &llm, false).await;
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn a_changed_brief_asks_again() {
    let (dir, ingest) = library();
    let llm = FakeLlm::answering(answer(&[("lib/shop/order.rb", "an order")]));
    infer(&dir, &ingest, &llm, false).await;

    infer_business_files(
        dir.path(),
        &ingest,
        "A Ruby library",
        &brief("Sells meals"),
        &llm,
        false,
    )
    .await
    .unwrap();

    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn a_hand_edited_file_is_kept_unless_forced() {
    let (dir, ingest) = library();
    let llm = FakeLlm::answering(answer(&[("lib/shop/order.rb", "an order")]));
    let mut map = infer(&dir, &ingest, &llm, false).await;
    map.entries.push(BusinessEntry {
        path: "lib/shop/pricing/".to_string(),
        reason: "mine".to_string(),
    });
    map.save(dir.path()).unwrap();

    let kept = infer(&dir, &ingest, &llm, false).await;
    assert_eq!(kept.entries.len(), 2);
    assert_eq!(llm.calls(), 1);

    let forced = infer(&dir, &ingest, &llm, true).await;
    assert_eq!(forced.entries.len(), 1);
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn an_unreadable_file_is_left_alone() {
    let (dir, ingest) = library();
    let path = Artifact::BusinessFiles.path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "entries: [not, valid").unwrap();
    let llm = FakeLlm::answering(answer(&[("lib/shop/order.rb", "an order")]));

    let map = infer(&dir, &ingest, &llm, false).await;

    assert!(map.is_empty());
    assert_eq!(llm.calls(), 0);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "entries: [not, valid"
    );
}

#[test]
fn files_expand_directories_without_repeats_and_within_the_bound() {
    let mut files: Vec<FileEntry> = (0..100)
        .map(|i| file(&format!("lib/big/f{i:03}.rb"), FileKind::Source))
        .collect();
    files.push(file("lib/big/test_x.rb", FileKind::Test));
    files.push(file("lib/other.rb", FileKind::Source));
    let map = BusinessMap {
        entries: vec![
            BusinessEntry {
                path: "lib/other.rb".into(),
                reason: String::new(),
            },
            // A hand-written directory without its slash works too.
            BusinessEntry {
                path: "lib/big".into(),
                reason: String::new(),
            },
            BusinessEntry {
                path: "lib/other.rb".into(),
                reason: String::new(),
            },
        ],
        ..BusinessMap::default()
    };

    let got = map.files(&files);

    assert_eq!(got.len(), MAX_FILES);
    assert_eq!(got[0], PathBuf::from("lib/other.rb"));
    assert_eq!(got[1], PathBuf::from("lib/big/f000.rb"));
    assert!(!got.iter().any(|p| p.ends_with("test_x.rb")));
}

#[test]
fn a_directory_name_does_not_match_a_sibling_with_the_same_prefix() {
    let files = vec![
        file("lib/shop/order.rb", FileKind::Source),
        file("lib/shopping/cart.rb", FileKind::Source),
    ];
    let map = BusinessMap {
        entries: vec![BusinessEntry {
            path: "lib/shop/".into(),
            reason: String::new(),
        }],
        ..BusinessMap::default()
    };

    assert_eq!(map.files(&files), [PathBuf::from("lib/shop/order.rb")]);
}

#[test]
fn the_prompt_stays_bounded_on_a_large_repository() {
    let dir = tempfile::tempdir().unwrap();
    let mut ingest = IngestResult {
        files: Vec::new(),
        history_by_path: std::collections::HashMap::new(),
        existing_docs: Vec::new(),
        commits: Vec::new(),
    };
    for d in 0..2_000 {
        for f in 0..5 {
            ingest.files.push(file(
                &format!("src/module{d:04}/part{f}.rb"),
                FileKind::Source,
            ));
        }
    }
    for d in 0..200 {
        ingest.existing_docs.push(ExistingDoc {
            path: PathBuf::from(format!("docs/{d}.md")),
            content: "# Title\n## Another\n".to_string(),
        });
    }

    let prompt = build_prompt(
        dir.path(),
        &ingest,
        "A big monolith",
        &ProductBrief::default(),
    );

    assert!(
        prompt.chars().count() < 60_000,
        "{}",
        prompt.chars().count()
    );
}

#[tokio::test]
async fn an_out_of_date_file_survives_a_failing_llm() {
    let (dir, mut ingest) = library();
    let llm = FakeLlm::answering(answer(&[("lib/shop/order.rb", "an order")]));
    let first = infer(&dir, &ingest, &llm, false).await;
    ingest
        .files
        .push(file("lib/billing/invoice.rb", FileKind::Source));
    let failing = FakeLlm::failing_after(0, |_| String::new());

    let map = infer(&dir, &ingest, &failing, false).await;

    assert_eq!(map.entries, first.entries);
}
