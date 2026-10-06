#!/usr/bin/env rust-script
//! Behavior test for `smoke.rs evaluate`: the verdict of a smoke test from the logs of two
//! `generate` runs and of `report`. No LLM needed. Usage: `just test-harness`.

use std::fs;
use std::path::Path;
use std::process::Command;

const GOOD_RUN1: &str = "Features:\n  ...\n\n12 file(s) written to docs.\n";
const GOOD_RUN2: &str = "No change.\n\n0 file(s) written to docs.\n";
const GOOD_REPORT: &str = "# Documentation debt report\n\n6 use cases\n";

fn main() {
    let dir = Path::new(file!()).parent().unwrap();
    let script = fs::canonicalize(dir.join("smoke.rs")).unwrap();
    let tmp = std::env::temp_dir().join(format!("smoke-test-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();

    let evaluate = |run1: &str, run2: &str, report: &str| -> (bool, String) {
        for (name, text) in [("run1", run1), ("run2", run2), ("report", report)] {
            fs::write(tmp.join(name), text).unwrap();
        }
        let out = Command::new("rust-script")
            .arg(&script)
            .arg("evaluate")
            .args(["run1", "run2", "report"].map(|n| tmp.join(n)))
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
        ("conclusive run", evaluate(GOOD_RUN1, GOOD_RUN2, GOOD_REPORT), true, ""),
        (
            "first run never reached the render",
            evaluate("Error: failed to build the repo map\n", GOOD_RUN2, GOOD_REPORT),
            false,
            "first run",
        ),
        (
            "warnings in the first run",
            evaluate(&format!(" WARN unit skipped\n{GOOD_RUN1}"), GOOD_RUN2, GOOD_REPORT),
            false,
            "warning",
        ),
        (
            "second run rewrote files",
            evaluate(GOOD_RUN1, "3 file(s) written to docs.\n", GOOD_REPORT),
            false,
            "second run",
        ),
        ("empty report", evaluate(GOOD_RUN1, GOOD_RUN2, "  \n"), false, "report"),
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
