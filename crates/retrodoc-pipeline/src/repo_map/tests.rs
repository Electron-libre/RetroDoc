use super::*;
use retrodoc_llm::{CompletionRequest, Role};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use retrodoc_ingest::{FileEntry, FileKind, IngestResult};
use retrodoc_llm::{CompletionResponse, LlmError};

use crate::testing::FakeLlm;

/// Answers a summary derived from the first line of the prompt, so that a
/// changed file gives a different summary.
fn counting_provider() -> FakeLlm {
    FakeLlm::replying(|_, request| {
        let first_line = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .and_then(|m| m.content.lines().next())
            .unwrap_or("");
        Ok(format!("summary of: {first_line}"))
    })
}

fn ingest_with_nested_files(root: &Path) -> IngestResult {
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::write(root.join("a/b/x.rs"), "fn x() {}").unwrap();
    std::fs::write(root.join("a/y.rs"), "fn y() {}").unwrap();

    IngestResult {
        files: vec![
            FileEntry {
                path: PathBuf::from("a/b/x.rs"),
                kind: FileKind::Source,
                size_bytes: 9,
            },
            FileEntry {
                path: PathBuf::from("a/y.rs"),
                kind: FileKind::Source,
                size_bytes: 9,
            },
        ],
        history_by_path: HashMap::new(),
        existing_docs: Vec::new(),
        commits: Vec::new(),
    }
}

#[tokio::test]
async fn builds_bottom_up_modules_for_nested_directories() {
    let dir = tempfile::tempdir().unwrap();
    let ingest = ingest_with_nested_files(dir.path());
    let provider = counting_provider();

    let map = build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
        .await
        .unwrap();

    assert_eq!(map.files.len(), 2);
    let module_paths: Vec<_> = map.modules.iter().map(|m| m.path.clone()).collect();
    assert!(module_paths.contains(&PathBuf::from("a")));
    assert!(module_paths.contains(&PathBuf::from("a/b")));

    // "a" aggregates its own file (y.rs) and sub-module "a/b" (x.rs).
    let module_a = map
        .modules
        .iter()
        .find(|m| m.path == Path::new("a"))
        .unwrap();
    assert_eq!(module_a.file_count, 2);
}

#[tokio::test]
async fn the_estimate_matches_the_calls_of_a_first_run_and_of_a_rerun() {
    let dir = tempfile::tempdir().unwrap();
    let ingest = ingest_with_nested_files(dir.path());
    let provider = counting_provider();

    let first = estimate_repo_map(dir.path(), &ingest, 0);
    assert_eq!((first.files, first.calls()), (2, 5));
    build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
        .await
        .unwrap();
    assert_eq!(provider.calls(), first.calls());

    assert_eq!(estimate_repo_map(dir.path(), &ingest, 0).calls(), 0);
    std::fs::write(dir.path().join("a/y.rs"), "fn y2() {}").unwrap();
    // y.rs, then "a" and the root; "a/b" is untouched.
    assert_eq!(estimate_repo_map(dir.path(), &ingest, 0).calls(), 3);
}

/// Records the highest number of calls in flight at the same time.
struct PeakProvider {
    in_flight: AtomicUsize,
    peak: AtomicUsize,
}

#[async_trait]
impl LlmProvider for PeakProvider {
    async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(CompletionResponse {
            content: "summary".to_string(),
            model: "test-model".to_string(),
            ..Default::default()
        })
    }
}

#[tokio::test]
async fn summaries_run_concurrently_up_to_the_limit_and_keep_their_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut ingest = ingest_with_nested_files(dir.path());
    for name in ["c", "d", "e"] {
        std::fs::write(dir.path().join(format!("a/{name}.rs")), name).unwrap();
        ingest.files.push(FileEntry {
            path: PathBuf::from(format!("a/{name}.rs")),
            kind: FileKind::Source,
            size_bytes: 1,
        });
    }
    let provider = PeakProvider {
        in_flight: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
    };

    let map = build_repo_map(
        dir.path(),
        &ingest,
        &provider,
        RepoMapOptions {
            concurrency: 3,
            batch_chars: 0,
        },
    )
    .await
    .unwrap();

    assert_eq!(provider.peak.load(Ordering::SeqCst), 3);
    let paths: Vec<_> = map.files.iter().map(|f| f.path.clone()).collect();
    let expected: Vec<_> = ingest.files.iter().map(|f| f.path.clone()).collect();
    assert_eq!(paths, expected);
    assert!(map.files.iter().all(|f| f.role_summary == "summary"));
    assert_eq!(map.modules.len(), 3);
}

#[test]
fn plan_batches_groups_small_files_and_isolates_big_ones() {
    // 4000 chars per request: a file over 1000 is never batched.
    let files = [(0, 100), (1, 200), (2, 2000), (3, 300), (4, 900), (5, 900)];
    assert_eq!(
        plan_batches(&files, 4000),
        vec![vec![0, 1], vec![2], vec![3, 4, 5]]
    );
    let many: Vec<_> = (0..10).map(|i| (i, 10)).collect();
    assert_eq!(plan_batches(&many, 4000).len(), 2); // 8 + 2
    assert_eq!(plan_batches(&many, 0).len(), 10); // batching off
}

#[tokio::test]
async fn small_files_share_a_request_and_missing_ones_fall_back() {
    let dir = tempfile::tempdir().unwrap();
    let ingest = ingest_with_nested_files(dir.path());
    let provider = FakeLlm::replying(|_, request| {
        // A batched request is answered for the first file only: the rest
        // must fall back to their own request.
        Ok(if request.messages[0].content.contains("\"summaries\"") {
            r#"{"summaries":[{"path":"a/b/x.rs","summary":"batched x"}]}"#.to_string()
        } else {
            "single".to_string()
        })
    });
    let options = RepoMapOptions {
        concurrency: 1,
        batch_chars: 4000,
    };
    let estimate = estimate_repo_map(dir.path(), &ingest, options.batch_chars);
    assert_eq!(estimate.file_calls, 1);

    let map = build_repo_map(dir.path(), &ingest, &provider, options)
        .await
        .unwrap();

    let summary_of = |p: &str| {
        map.files
            .iter()
            .find(|f| f.path == Path::new(p))
            .unwrap()
            .role_summary
            .clone()
    };
    assert_eq!(summary_of("a/b/x.rs"), "batched x");
    assert_eq!(summary_of("a/y.rs"), "single");
    // 1 batched request + 1 fallback for y.rs + 3 folders.
    assert_eq!(provider.calls(), 5);
}

#[tokio::test]
async fn unchanged_files_are_not_re_summarized_on_second_run() {
    let dir = tempfile::tempdir().unwrap();
    let ingest = ingest_with_nested_files(dir.path());
    let provider = counting_provider();

    build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
        .await
        .unwrap();
    // 2 files + 3 modules ("a/b", "a", root "") = 5 calls on the first run.
    let calls_after_first_run = provider.calls();
    assert_eq!(calls_after_first_run, 5);

    build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
        .await
        .unwrap();
    let calls_after_second_run = provider.calls();

    // File and module summaries are both served from the cache
    // (unchanged content, hence unchanged module listings).
    assert_eq!(calls_after_second_run, calls_after_first_run);
}

#[tokio::test]
async fn a_changed_file_only_invalidates_its_folder_and_ancestors() {
    let dir = tempfile::tempdir().unwrap();
    let ingest = ingest_with_nested_files(dir.path());
    std::fs::create_dir_all(dir.path().join("c")).unwrap();
    std::fs::write(dir.path().join("c/z.rs"), "fn z() {}").unwrap();
    let mut ingest = ingest;
    ingest.files.push(FileEntry {
        path: PathBuf::from("c/z.rs"),
        kind: FileKind::Source,
        size_bytes: 9,
    });
    let provider = FakeLlm::replying(|n, _| Ok(format!("summary #{n}")));
    build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
        .await
        .unwrap();
    let first = provider.calls();

    std::fs::write(dir.path().join("a/b/x.rs"), "fn x2() {}").unwrap();
    build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
        .await
        .unwrap();
    // Each answer is unique, so a new x.rs summary changes its parents'
    // listings: x.rs, then "a/b", "a" and the root; "c" is untouched.
    assert_eq!(provider.calls() - first, 4);
}

#[tokio::test]
async fn a_failure_partway_through_the_file_loop_keeps_earlier_summaries_cached() {
    let dir = tempfile::tempdir().unwrap();
    let ingest = ingest_with_nested_files(dir.path());
    // Only the first file summarization call succeeds; the second one
    // (and the run as a whole) fails.
    let provider = FakeLlm::failing_after(1, |n| format!("summary #{n}"));

    let result = build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default()).await;
    assert!(result.is_err());

    // The summary computed before the failure was still persisted to
    // disk, not discarded along with the run.
    let cache = RepoMapCache::load(dir.path());
    let cached_paths: Vec<_> = [Path::new("a/b/x.rs"), Path::new("a/y.rs")]
        .into_iter()
        .filter(|p| {
            cache
                .get(
                    p,
                    &hash_content(&std::fs::read_to_string(dir.path().join(p)).unwrap()),
                )
                .is_some()
        })
        .collect();
    assert_eq!(cached_paths.len(), 1);
}
