#!/usr/bin/env rust-script
//! Smoke test of `retrodoc generate` against a real repository with a local LLM.
//!
//! - `smoke.rs run <repo> [--model M] [--base-url U]`: clones `<repo>` (committed HEAD only, the
//!   original is never touched) into /tmp, points it at the local LLM, runs `generate` twice and
//!   `report`, then prints the verdict.
//! - `smoke.rs evaluate <run1.log> <run2.log> <report.txt>`: only the verdict, from saved logs.
//!
//! Usage: `just smoke <repo>`. A run takes minutes to hours: start it in the background.

use std::fs::{self, File};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const DEFAULT_MODEL: &str = "qwen3.6:35b-a3b";
const DEFAULT_BASE_URL: &str = "http://localhost:11435/v1/chat/completions";

/// The smoke test is conclusive when every returned list is empty.
fn evaluate(run1: &str, run2: &str, report: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if !run1.contains("file(s) written to") {
        problems.push("first run did not reach the render step (generate failed or stopped)".into());
    }
    if !run2.lines().any(|l| l.starts_with("0 file(s) written to")) {
        problems.push("second run is not a no-op: it wrote files (cache or render not idempotent)".into());
    }
    if report.trim().is_empty() {
        problems.push("report is empty".into());
    }
    for (name, log) in [("first", run1), ("second", run2)] {
        let warnings: Vec<&str> = log.lines().filter(|l| l.contains(" WARN ")).collect();
        if !warnings.is_empty() {
            problems.push(format!("{} warning(s) in the {name} run, e.g. {}", warnings.len(), warnings[0].trim()));
        }
    }
    problems
}

fn verdict(problems: &[String]) -> i32 {
    if problems.is_empty() {
        println!("SMOKE TEST CONCLUSIVE: generate finished, no warning, rerun is a no-op, report produced");
        return 0;
    }
    for p in problems {
        println!("FAIL - {p}");
    }
    1
}

fn project_root() -> PathBuf {
    fs::canonicalize(Path::new(file!()).parent().unwrap().join("../../..")).unwrap()
}

fn die(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2);
}

fn run(args: &[String]) -> i32 {
    let Some(target) = args.first() else { die("usage: smoke.rs run <repo> [--model M] [--base-url U]") };
    let option = |name: &str, default: &str| {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or(default.into())
    };
    let (model, base_url) = (option("--model", DEFAULT_MODEL), option("--base-url", DEFAULT_BASE_URL));

    // The LLM must answer before we spend minutes building.
    let host = base_url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or("");
    let reachable = host.to_socket_addrs().ok().and_then(|mut a| {
        a.find_map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(3)).ok())
    });
    if reachable.is_none() {
        die(&format!("no LLM server answers on {host}; start it or pass --base-url"));
    }

    let name = Path::new(target).file_name().unwrap().to_string_lossy().into_owned();
    let clone = std::env::temp_dir().join(format!("retrodoc-smoke-{name}"));
    if clone.exists() {
        fs::remove_dir_all(&clone).expect("remove the previous smoke clone");
    }
    let cloned = Command::new("git").args(["clone", "-q", target]).arg(&clone).status().unwrap();
    if !cloned.success() {
        die("git clone of the target failed (is it a Git repository?)");
    }

    let root = project_root();
    let retrodoc = |extra: &[&str], log: &Path| -> bool {
        let out = File::create(log).unwrap();
        Command::new("cargo")
            .args(["run", "-q", "-p", "retrodoc-cli", "--"])
            .args(extra)
            .arg("--path")
            .arg(&clone)
            .current_dir(&root)
            .env("OPENROUTER_API_KEY", "unused")
            .stdout(out.try_clone().unwrap())
            .stderr(out)
            .status()
            .unwrap()
            .success()
    };
    let log = |suffix: &str| std::env::temp_dir().join(format!("retrodoc-smoke-{name}.{suffix}"));
    let (log1, log2, rep) = (log("run1.log"), log("run2.log"), log("report.txt"));

    if !retrodoc(&["init"], &rep) {
        die("retrodoc init failed");
    }
    let toml_path = clone.join("retrodoc.toml");
    let kept: Vec<String> = fs::read_to_string(&toml_path)
        .unwrap()
        .lines()
        .filter(|l| !["model =", "base_url =", "reasoning_effort ="].iter().any(|k| l.starts_with(k)))
        .map(String::from)
        .collect();
    let patched = kept.join("\n").replacen(
        "[llm]",
        &format!("[llm]\nmodel = \"{model}\"\nbase_url = \"{base_url}\"\nreasoning_effort = \"none\""),
        1,
    );
    fs::write(&toml_path, patched).unwrap();

    println!("clone: {} | logs: {} {}", clone.display(), log1.display(), log2.display());
    println!("first run (builds everything)…");
    let ok1 = retrodoc(&["generate"], &log1);
    println!("second run (must be a no-op)…");
    let ok2 = retrodoc(&["generate"], &log2);
    let okr = retrodoc(&["report"], &rep);

    let read = |p: &Path| fs::read_to_string(p).unwrap_or_default();
    let mut problems = evaluate(&read(&log1), &read(&log2), &read(&rep));
    for (ok, what) in [(ok1, "first generate"), (ok2, "second generate"), (okr, "report")] {
        if !ok {
            problems.insert(0, format!("{what} exited with an error (see the logs)"));
        }
    }
    verdict(&problems)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("evaluate") if args.len() == 4 => {
            let read = |i: usize| fs::read_to_string(&args[i]).unwrap_or_else(|e| die(&format!("{}: {e}", args[i])));
            verdict(&evaluate(&read(1), &read(2), &read(3)))
        }
        Some("run") => run(&args[1..]),
        _ => die("usage: smoke.rs run <repo> | smoke.rs evaluate <run1> <run2> <report>"),
    };
    std::process::exit(code);
}
