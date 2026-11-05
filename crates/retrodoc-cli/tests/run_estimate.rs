//! The estimate of the calls and tokens of a `generate`, printed before its first LLM call
//! (`specs/run-estimate.md`).

mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

use support::{llm_section, repo_with_config, run, FakeLlm};

const FILES: &[(&str, &str)] = &[
    ("app/order.rb", "class Order; end\n"),
    ("app/customer.rb", "class Customer; end\n"),
];

fn repo(llm: &FakeLlm) -> tempfile::TempDir {
    repo_with_config(
        FILES,
        &format!("{}structured_output = false\n", llm_section(llm)),
    )
}

fn stdout(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

fn tree(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = walk(dir)
        .into_iter()
        .filter(|p| !p.starts_with(dir.join(".git")))
        .collect();
    paths.sort();
    paths
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                let mut inner = walk(&path);
                inner.push(path);
                inner
            } else {
                vec![path]
            }
        })
        .collect()
}

// spec: EST-2, EST-3
#[test]
fn estimate_prints_the_estimate_and_stops_before_any_call_or_write() {
    let llm = FakeLlm::start(|_| r#"{"domains":[]}"#.to_string());
    let dir = repo(&llm);
    let before = tree(dir.path());

    let out = stdout(&run(dir.path(), &["generate", "--estimate"]));

    assert!(out.contains("LLM estimate: "), "{out}");
    assert!(out.contains("(2 source file(s) to read)"), "{out}");
    assert!(llm.calls().is_empty(), "{out}");
    assert_eq!(tree(dir.path()), before);
    assert!(!out.contains("LLM usage"), "{out}");
}

// spec: EST-1, EST-9
#[test]
fn generate_prints_the_estimate_before_its_first_call() {
    let llm = FakeLlm::start(|_| r#"{"domains":[]}"#.to_string());
    let dir = repo(&llm);

    let out = stdout(&run(dir.path(), &["generate", "--no-confidence"]));

    let estimate = out.find("LLM estimate: ").expect("an estimate");
    let first_pass = out
        .find("Writing the product brief")
        .expect("the brief written");
    assert!(estimate < first_pass, "{out}");
    let block = &out[estimate..first_pass];
    for pass in ["brief", "use-cases", "rules"] {
        assert!(block.contains(&format!("  {pass} ")), "{pass}: {block}");
    }
    assert!(
        block.contains("confidence") && block.contains("off in this run"),
        "{block}"
    );
    assert!(
        block.contains("Extrapolated from the calls and tokens measured"),
        "{block}"
    );
    assert!(
        block.contains("Run on fake, not the measured model: every pass."),
        "{block}"
    );
}

// spec: EST-7, EST-3
#[test]
fn a_rerun_with_nothing_changed_is_estimated_at_zero() {
    let llm = FakeLlm::start(|_| r#"{"domains":[]}"#.to_string());
    let dir = repo(&llm);
    stdout(&run(dir.path(), &["generate", "--no-confidence"]));

    let out = stdout(&run(dir.path(), &["generate", "--estimate"]));

    assert!(
        out.contains(
            "LLM estimate: 0 call(s), 0 tokens in, 0 tokens out (0 of 2 source file(s) changed"
        ),
        "{out}"
    );
}

// spec: EST-8
#[test]
fn a_rerun_is_estimated_on_the_changed_files() {
    let llm = FakeLlm::start(|_| r#"{"domains":[]}"#.to_string());
    let dir = repo(&llm);
    stdout(&run(dir.path(), &["generate", "--no-confidence"]));
    std::fs::write(
        dir.path().join("app/order.rb"),
        "class Order; def total; end; end\n",
    )
    .unwrap();

    let out = stdout(&run(dir.path(), &["generate", "--estimate"]));

    assert!(out.contains("(1 of 2 source file(s) changed"), "{out}");
    assert!(!out.contains("LLM estimate: 0 call(s)"), "{out}");
}
