#!/usr/bin/env rust-script
//! Behavior test for `smoke.rs evaluate`: the verdict of a smoke test from the logs of two
//! `generate` runs and of `report`. No LLM needed. Usage: `just test-harness`.

use std::fs;
use std::path::Path;
use std::process::Command;

const GOOD_RUN1: &str = "Features:\n  ...\n\n12 file(s) written to docs.\n";
const GOOD_RUN2: &str = "No change.\n\n0 file(s) written to docs.\n\nLLM usage: no call (everything was reused from the cache).\n";
/// A second run that spent tokens: the recap line `generate` prints.
const CALLING_RUN2: &str = "0 file(s) written to docs.\n\nLLM usage: 2 call(s), 922 tokens in, 20 tokens out, 3s.\n";
const GOOD_REPORT: &str = "# Documentation debt report\n\n6 use cases\n";

/// `usage.json` as the CLI writes it: the first `generate` called the LLM, the second did not (a run
/// without any call is not saved).
const GOOD_USAGE: &str = r#"{"runs": [
  {"command": "generate", "finished_at": "t1", "wall_ms": 9, "passes": [
    {"name": "roles", "wall_ms": 1, "models": [{"model": "m", "calls": 1, "calls_without_usage": 0, "prompt_tokens": 10, "completion_tokens": 5}]},
    {"name": "use-cases", "wall_ms": 1, "models": [{"model": "m", "calls": 6, "calls_without_usage": 0, "prompt_tokens": 10, "completion_tokens": 5}]}]}]}"#;

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
            "second run called the LLM again (passes named from usage.json)",
            evaluate(GOOD_RUN1, CALLING_RUN2, GOOD_REPORT, Some(RERUN_USAGE)),
            false,
            "use-cases: 2",
        ),
        (
            "second run called the LLM again, usage.json missing",
            evaluate(GOOD_RUN1, CALLING_RUN2, GOOD_REPORT, None),
            false,
            "called the LLM",
        ),
        (
            "second run printed no recap",
            evaluate(GOOD_RUN1, "0 file(s) written to docs.\n", GOOD_REPORT, Some(GOOD_USAGE)),
            false,
            "recap",
        ),
        ("usage.json missing is fine when the recap says no call", evaluate(GOOD_RUN1, GOOD_RUN2, GOOD_REPORT, None), true, ""),
        ("empty report", evaluate(GOOD_RUN1, GOOD_RUN2, "  \n", Some(GOOD_USAGE)), false, "report"),
    ];

    // The `[llm]` section written for the local model: a timeout that fits it, replacing any other.
    let toml = "[llm]\nprovider = \"openrouter\"\nmodel = \"x/y\"\ntimeout_secs = 30\n\n[llm.passes.x]\nmodel = \"keep/me\"\n\n[pipeline]\nrules = false\n";
    fs::write(tmp.join("retrodoc.toml"), toml).unwrap();
    let patched = Command::new("rust-script")
        .arg(&script)
        .args(["patch-config"])
        .arg(tmp.join("retrodoc.toml"))
        .args(["local:model", "http://h:1/v1/chat/completions"])
        .output()
        .expect("rust-script must be installed");
    let patched = String::from_utf8_lossy(&patched.stdout).into_owned();
    let config_ok = patched.contains("timeout_secs = 900\n")
        && patched.matches("timeout_secs").count() == 1
        && patched.matches("model =").count() == 2
        && patched.contains("model = \"keep/me\"")
        && patched.contains("model = \"local:model\"")
        && patched.contains("[pipeline]\nrules = false");
    println!("{} - written config has one timeout the local model can meet", if config_ok { "ok  " } else { "FAIL" });
    if !config_ok {
        println!("       got: {patched}");
    }

    // `--local` names a local server: its model and URL (explicit flags win), and the command that
    // starts it when nothing answers.
    let smoke = |args: &[&str]| -> (bool, String) {
        let out = Command::new("rust-script").arg(&script).args(args).output().expect("rust-script must be installed");
        (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
    };
    let local_cases: Vec<(&str, (bool, String), bool, &str)> = vec![
        ("the default local target is the Ollama iGPU instance", smoke(&["llm-target"]), true, "qwen3.6:35b-a3b\nhttp://localhost:11435/v1/chat/completions\n"),
        (
            "--local hybrid targets the llama-server",
            smoke(&["llm-target", "--local", "hybrid"]),
            true,
            "qwen3.6:35b-a3b-hybride\nhttp://127.0.0.1:11437/v1/chat/completions\n",
        ),
        ("--model and --base-url win over --local", smoke(&["llm-target", "--local", "hybrid", "--model", "m", "--base-url", "u"]), true, "m\nu\n\n"),
        ("--local names the command that starts its server", smoke(&["llm-target", "--local", "hybrid"]), true, "\nsystemctl --user start llama-hybride\n"),
        ("an unknown --local is an error", smoke(&["llm-target", "--local", "nope"]), false, "--local must be `igpu` or `hybrid`"),
        ("--local without a value is an error", smoke(&["llm-target", "--local"]), false, "--local must be `igpu` or `hybrid`"),
        (
            "nothing answers on an explicit --base-url: no service of another server is named",
            smoke(&["run", "/nonexistent", "--local", "hybrid", "--base-url", "http://127.0.0.1:1/v1/chat/completions"]),
            false,
            "no LLM server answers on 127.0.0.1:1; start it or pass --base-url",
        ),
    ];
    let mut failed = !config_ok;
    for (name, (passed, output), should_pass, expected) in local_cases {
        let ok = passed == should_pass && output.contains(expected);
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            println!("       got pass={passed}, output: {}", output.trim());
        }
        failed |= !ok;
    }
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
