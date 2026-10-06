#!/usr/bin/env rust-script
//! Behavior test for `smoke.rs evaluate`: the verdict of a smoke test from the logs of two
//! `generate` runs and of `report`. No LLM needed. Usage: `just test-harness`.

use std::fs;
use std::path::Path;
use std::process::Command;

const GOOD_RUN1: &str = "Features:\n  ...\n\n12 file(s) written to docs.\n";
const GOOD_RUN2: &str = "No change.\n\n0 file(s) written to docs.\n";
const GOOD_REPORT: &str = "# Documentation debt report\n\n6 use cases\n";

/// `usage.json` as the CLI writes it: the first `generate` called the LLM, the second did not.
const GOOD_USAGE: &str = r#"{"runs": [
  {"command": "generate", "finished_at": "t1", "wall_ms": 9, "passes": [
    {"name": "roles", "wall_ms": 1, "models": [{"model": "m", "calls": 1, "calls_without_usage": 0, "prompt_tokens": 10, "completion_tokens": 5}]},
    {"name": "use-cases", "wall_ms": 1, "models": [{"model": "m", "calls": 6, "calls_without_usage": 0, "prompt_tokens": 10, "completion_tokens": 5}]}]},
  {"command": "generate", "finished_at": "t2", "wall_ms": 1, "passes": []}]}"#;

/// The second `generate` asked the LLM again for a feature, in the `use-cases` pass.
const RERUN_USAGE: &str = r#"{"runs": [
  {"command": "generate", "finished_at": "t1", "wall_ms": 9, "passes": [
    {"name": "use-cases", "wall_ms": 1, "models": [{"model": "m", "calls": 6, "calls_without_usage": 0, "prompt_tokens": 10, "completion_tokens": 5}]}]},
  {"command": "generate", "finished_at": "t2", "wall_ms": 1, "passes": [
    {"name": "roles", "wall_ms": 1, "models": [{"model": "m", "calls": 0, "calls_without_usage": 0, "prompt_tokens": 0, "completion_tokens": 0}]},
    {"name": "use-cases", "wall_ms": 1, "models": [{"model": "m", "calls": 2, "calls_without_usage": 0, "prompt_tokens": 922, "completion_tokens": 20}]}]}]}"#;

fn main() {
    let dir = Path::new(file!()).parent().unwrap();
    let script = fs::canonicalize(dir.join("smoke.rs")).unwrap();
    let tmp = std::env::temp_dir().join(format!("smoke-test-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();

    // `usage`: the content of `usage.json`, or `None` when the file does not exist.
    let evaluate = |run1: &str, run2: &str, report: &str, usage: Option<&str>| -> (bool, String) {
        for (name, text) in [("run1", run1), ("run2", run2), ("report", report)] {
            fs::write(tmp.join(name), text).unwrap();
        }
        let _ = fs::remove_file(tmp.join("usage.json"));
        if let Some(usage) = usage {
            fs::write(tmp.join("usage.json"), usage).unwrap();
        }
        let out = Command::new("rust-script")
            .arg(&script)
            .arg("evaluate")
            .args(["run1", "run2", "report", "usage.json"].map(|n| tmp.join(n)))
            .output()
            .expect("rust-script must be installed");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (out.status.success(), text)
    };

    let cases: Vec<(&str, (bool, String), bool, &str)> = vec![
        ("conclusive run", evaluate(GOOD_RUN1, GOOD_RUN2, GOOD_REPORT, Some(GOOD_USAGE)), true, ""),
        (
            "first run never reached the render",
            evaluate("Error: failed to build the repo map\n", GOOD_RUN2, GOOD_REPORT, Some(GOOD_USAGE)),
            false,
            "first run",
        ),
        (
            "warnings in the first run",
            evaluate(&format!(" WARN unit skipped\n{GOOD_RUN1}"), GOOD_RUN2, GOOD_REPORT, Some(GOOD_USAGE)),
            false,
            "warning",
        ),
        (
            "colored warning (tracing writes ANSI codes to a redirected stderr)",
            evaluate(&format!("\x1b[33m WARN\x1b[0m \x1b[2mretrodoc\x1b[0m: unit skipped\n{GOOD_RUN1}"), GOOD_RUN2, GOOD_REPORT, Some(GOOD_USAGE)),
            false,
            "warning",
        ),
        (
            "second run rewrote files",
            evaluate(GOOD_RUN1, "3 file(s) written to docs.\n", GOOD_REPORT, Some(GOOD_USAGE)),
            false,
            "second run",
        ),
        (
            "second run called the LLM again",
            evaluate(GOOD_RUN1, GOOD_RUN2, GOOD_REPORT, Some(RERUN_USAGE)),
            false,
            "use-cases",
        ),
        (
            "usage.json is missing",
            evaluate(GOOD_RUN1, GOOD_RUN2, GOOD_REPORT, None),
            false,
            "usage.json",
        ),
        (
            "usage.json is unreadable",
            evaluate(GOOD_RUN1, GOOD_RUN2, GOOD_REPORT, Some("not json")),
            false,
            "usage.json",
        ),
        ("empty report", evaluate(GOOD_RUN1, GOOD_RUN2, "  \n", Some(GOOD_USAGE)), false, "report"),
    ];

    let mut failed = false;
    for (name, (passed, output), should_pass, hint) in cases {
        let ok = passed == should_pass && (should_pass || output.contains(hint));
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            println!("       got pass={passed}, output: {}", output.trim());
        }
        failed |= !ok;
    }
    let _ = fs::remove_dir_all(&tmp);
    std::process::exit(i32::from(failed));
}
