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

// spec: EST-3
#[test]
fn the_budget_does_not_cut_the_files_of_the_surface_path() {
    let llm = FakeLlm::start(|_| r#"{"domains":[]}"#.to_string());
    let dir = repo(&llm);

    let out = stdout(&run(
        dir.path(),
        &["generate", "--estimate", "--max-files", "1"],
    ));

    assert!(out.contains("(2 source file(s) to read)"), "{out}");
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
        block.contains("Extrapolated from the calls and tokens of saved runs"),
        "{block}"
    );
    assert!(block.contains("this run asks fake."), "{block}");
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

// spec: TOK-6
#[test]
fn the_estimate_is_saved_with_the_run_in_the_usage_history() {
    let llm = FakeLlm::start(|_| r#"{"domains":[]}"#.to_string());
    let dir = repo(&llm);

    stdout(&run(dir.path(), &["generate", "--no-confidence"]));

    let raw = std::fs::read_to_string(dir.path().join(".retrodoc/cache/usage.json")).unwrap();
    let history: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let estimate = &history["runs"][0]["estimate"];
    assert_eq!(estimate["source_files"], 2, "{raw}");
    let passes = estimate["passes"].as_array().expect("the passes estimated");
    let confidence = passes
        .iter()
        .find(|p| p["pass"] == "confidence")
        .expect("confidence");
    assert_eq!(confidence["basis"], "off", "{raw}");
    let use_cases = passes
        .iter()
        .find(|p| p["pass"] == "use-cases")
        .expect("use-cases");
    assert!(use_cases["calls"]["high"].as_u64().unwrap() > 0, "{raw}");
}

// spec: TOK-7
#[test]
fn the_recap_of_generate_sets_the_passes_against_the_estimate() {
    let llm = FakeLlm::start(|_| r#"{"domains":[]}"#.to_string());
    let dir = repo(&llm);

    let out = stdout(&run(dir.path(), &["generate", "--no-confidence"]));

    let recap = &out[out.find("LLM usage: ").expect("a recap")..];
    let against = &recap[recap.find("Against the estimate:").expect("the comparison")..];
    assert!(against.contains("  brief "), "{against}");
    assert!(!against.contains("confidence"), "{against}");
}
