//! The run history (`specs/run-history.md`): a `generate` that finishes leaves a snapshot of its
//! figures and sections under `.retrodoc/cache/history/`, none when nothing changed, and never
//! touches the earlier ones.

mod support;

use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use support::{generate, git, run, warnings, App, FakeLlm};

fn shop() -> (tempfile::TempDir, FakeLlm) {
    let app = App {
        stack: "Ruby",
        source_extensions: vec!["rb"],
        chunk_boundaries: Vec::new(),
        business: "app/",
        entry: "app/orders_controller.rb",
        model: "app/order.rb",
    };
    let llm = FakeLlm::start(move |call| app.answer(call));
    let dir = support::repo(
        &[
            (
                "app/orders_controller.rb",
                "get '/orders' do\n  Order.all\nend\n",
            ),
            ("app/order.rb", "class Order\nend\n"),
        ],
        &llm,
    );
    (dir, llm)
}

fn succeeded(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn history(dir: &Path) -> PathBuf {
    dir.join(".retrodoc/cache/history")
}

/// The files of the history, by name.
fn snapshots(dir: &Path) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = std::fs::read_dir(history(dir))
        .map(|entries| {
            entries
                .map(|e| {
                    let e = e.unwrap();
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        std::fs::read_to_string(e.path()).unwrap(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

fn head(dir: &Path) -> String {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

// spec: RHI-1, RHI-2, RHI-6, RHI-7
#[test]
fn a_generate_leaves_a_snapshot_of_its_commit_cost_and_sections() {
    let (dir, _llm) = shop();

    succeeded(&generate(dir.path()));

    let files = snapshots(dir.path());
    assert_eq!(files.len(), 1, "{files:?}");
    let (name, text) = &files[0];
    assert!(
        name.len() == "20261104T100000Z.json".len() && name.ends_with("Z.json"),
        "{name}"
    );
    let snapshot: Value = serde_json::from_str(text).unwrap();
    assert_eq!(snapshot["commit"], head(dir.path()).as_str());
    assert!(snapshot["finished_at"].as_str().unwrap().starts_with("20"));
    assert!(snapshot["cost"]["calls"].as_u64().unwrap() > 0, "{text}");
    let ids: Vec<(&str, &str)> = snapshot["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["kind"].as_str().unwrap(), s["id"].as_str().unwrap()))
        .collect();
    assert_eq!(
        ids,
        [
            ("domain", "orders"),
            ("feature", "list-orders"),
            ("use_case", "list-orders/list-orders"),
        ]
    );
}

// spec: RHI-9, RHI-11
#[test]
fn a_rerun_that_changes_nothing_writes_no_snapshot_and_force_keeps_them() {
    let (dir, _llm) = shop();
    succeeded(&generate(dir.path()));
    let first = snapshots(dir.path());
    assert_eq!(first.len(), 1);

    succeeded(&generate(dir.path()));
    // A new commit that changes no section nor figure.
    git(
        dir.path(),
        &["commit", "-q", "--allow-empty", "-m", "Say nothing"],
    );
    succeeded(&generate(dir.path()));
    succeeded(&run(dir.path(), &["generate", "--force"]));

    assert_eq!(snapshots(dir.path()), first);
}

// spec: RHI-10
#[test]
fn a_dry_run_writes_no_snapshot() {
    let (dir, _llm) = shop();

    succeeded(&run(dir.path(), &["generate", "--dry-run"]));

    assert_eq!(snapshots(dir.path()), []);
}

// spec: RHI-10
#[test]
fn a_generate_that_fails_after_a_run_writes_no_snapshot() {
    let (dir, llm) = shop();
    succeeded(&generate(dir.path()));
    // Any snapshot of the docs the first run left would now be new.
    std::fs::remove_dir_all(history(dir.path())).unwrap();
    let failing = FakeLlm::failing("400 Bad Request");
    let config = dir.path().join("retrodoc.toml");
    let raw = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, raw.replace(&llm.url, &failing.url)).unwrap();
    std::fs::write(
        dir.path().join("app/order.rb"),
        "class Order\n  def total; end\nend\n",
    )
    .unwrap();

    let output = generate(dir.path());

    assert!(!output.status.success());
    // The docs of the first run are still there to snapshot.
    assert!(dir.path().join(".retrodoc/cache/features.yaml").exists());
    assert_eq!(snapshots(dir.path()), []);
}

// spec: RHI-2
#[test]
fn the_snapshot_holds_the_commit_the_run_started_on() {
    let repo: Arc<Mutex<Option<PathBuf>>> = Arc::default();
    let app = App {
        stack: "Ruby",
        source_extensions: vec!["rb"],
        chunk_boundaries: Vec::new(),
        business: "app/",
        entry: "app/orders_controller.rb",
        model: "app/order.rb",
    };
    let during = Arc::clone(&repo);
    // A commit made while the run asks the model.
    let llm = FakeLlm::start(move |call| {
        if let Some(dir) = during.lock().unwrap().take() {
            git(&dir, &["commit", "-q", "--allow-empty", "-m", "Meanwhile"]);
        }
        app.answer(call)
    });
    let dir = support::repo(
        &[
            (
                "app/orders_controller.rb",
                "get '/orders' do\n  Order.all\nend\n",
            ),
            ("app/order.rb", "class Order\nend\n"),
        ],
        &llm,
    );
    let before = head(dir.path());
    *repo.lock().unwrap() = Some(dir.path().to_path_buf());

    succeeded(&generate(dir.path()));

    assert_ne!(head(dir.path()), before);
    let (_, text) = &snapshots(dir.path())[0];
    let snapshot: Value = serde_json::from_str(text).unwrap();
    assert_eq!(snapshot["commit"], before.as_str());
}

// spec: RHI-12
#[test]
fn a_snapshot_that_cannot_be_written_only_warns() {
    let (dir, _llm) = shop();
    std::fs::create_dir_all(dir.path().join(".retrodoc/cache")).unwrap();
    // A file where the folder of the history goes.
    std::fs::write(history(dir.path()), "").unwrap();

    let output = generate(dir.path());

    succeeded(&output);
    let warnings = warnings(&output);
    assert!(
        warnings.iter().any(|w| w.contains("snapshot of the run")),
        "{warnings:?}"
    );
}
