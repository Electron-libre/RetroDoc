//! `source-hashes.json`: written by a `generate` that went through every pass, left alone by one
//! that failed, so the MCP freshness check never trusts hashes the docs were not built from.

mod support;

use std::path::Path;
use std::process::Output;

use support::{llm_section, repo_with_config, run, FakeLlm};

fn repo(llm: &FakeLlm) -> tempfile::TempDir {
    repo_with_config(
        &[("app.rb", "class Order; end\n")],
        &format!("{}structured_output = false\n", llm_section(llm)),
    )
}

fn generate(dir: &Path) -> Output {
    run(dir, &["generate", "--no-confidence"])
}

fn saved(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(".retrodoc/cache/source-hashes.json")).ok()
}

#[test]
fn a_failed_run_leaves_the_hashes_of_the_last_complete_run() {
    let llm = FakeLlm::failing("400 Bad Request");
    let dir = repo(&llm);
    let cache = dir.path().join(".retrodoc/cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join("source-hashes.json"),
        r#"{"files":{"app.rb":"old"}}"#,
    )
    .unwrap();

    let output = generate(dir.path());

    assert!(!output.status.success());
    assert_eq!(
        saved(dir.path()).as_deref(),
        Some(r#"{"files":{"app.rb":"old"}}"#)
    );
}

#[test]
fn a_complete_run_records_the_hash_of_the_source_files() {
    let llm = FakeLlm::start(|_| r#"{"domains":[]}"#.to_string());
    let dir = repo(&llm);

    let output = generate(dir.path());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "{stdout}");
    let hashes = saved(dir.path()).expect("source-hashes.json written");
    assert!(hashes.contains("app.rb"), "{hashes}");
}
