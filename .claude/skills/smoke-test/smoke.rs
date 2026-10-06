#!/usr/bin/env rust-script
//! Smoke test of `retrodoc generate` against a real repository with a local LLM.
//!
//! - `smoke.rs run <repo> [--model M] [--base-url U]`: clones `<repo>` (committed HEAD only, the
//!   original is never touched) into /tmp, points it at the local LLM, runs `generate` twice and
//!   `report`, then prints the verdict.
//! - `smoke.rs evaluate <run1.log> <run2.log> <report.txt> <usage.json>`: only the verdict, from saved
//!   logs and the `.retrodoc/cache/usage.json` of the clone.
//!
//! Usage: `just smoke <repo>`. A run takes minutes to hours: start it in the background.

use std::fs::{self, File};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const DEFAULT_MODEL: &str = "qwen3.6:35b-a3b";
const DEFAULT_BASE_URL: &str = "http://localhost:11435/v1/chat/completions";

/// `line` without its ANSI escape sequences (`ESC [ … letter`), which `tracing` writes around the level.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip `[`, parameters and intermediates, up to the final letter.
            for end in chars.by_ref() {
                if end.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Every `"key": value` of `json` as `(position, value)`, the value being a string or a number. Enough
/// for `usage.json` (written by serde, no nesting trick), and keeps the script free of dependencies.
fn values(json: &str, key: &str) -> Vec<(usize, String)> {
    let needle = format!("\"{key}\"");
    let mut found = Vec::new();
    for (at, _) in json.match_indices(&needle) {
        let rest = json[at + needle.len()..].trim_start();
        let Some(rest) = rest.strip_prefix(':') else { continue };
        let rest = rest.trim_start();
        let value = match rest.strip_prefix('"') {
            Some(quoted) => quoted.split('"').next().unwrap_or_default(),
            None => rest.split(|c: char| !c.is_ascii_digit()).next().unwrap_or_default(),
        };
        found.push((at, value.to_string()));
    }
    found
}

/// LLM calls per pass of the last `generate` recorded in `usage.json` (the second run of the smoke test).
fn last_generate_calls(usage: &str) -> Result<Vec<(String, u64)>, String> {
    let commands = values(usage, "command");
    let Some(last) = commands.iter().rposition(|(_, c)| c == "generate") else {
        return Err("no `generate` run in it".into());
    };
    let start = commands[last].0;
    let end = commands.get(last + 1).map_or(usage.len(), |(at, _)| *at);
    let run = &usage[start..end];
    let mut events: Vec<(usize, bool, String)> = values(run, "name").into_iter().map(|(at, v)| (at, true, v)).collect();
    events.extend(values(run, "calls").into_iter().map(|(at, v)| (at, false, v)));
    events.sort_by_key(|(at, ..)| *at);

    let mut passes: Vec<(String, u64)> = Vec::new();
    for (_, is_name, value) in events {
        if is_name {
            passes.push((value, 0));
        } else {
            let calls = value.parse::<u64>().map_err(|_| format!("`calls` is not a number: {value:?}"))?;
            passes.last_mut().ok_or("`calls` before any pass name")?.1 += calls;
        }
    }
    Ok(passes)
}

/// Problem with the LLM calls of the second run, if any. `usage` is the content of `usage.json`, `None`
/// when the file does not exist: without it the criterion can't be checked, which is a failure too.
fn rerun_calls_problem(usage: Option<&str>) -> Option<String> {
    let Some(usage) = usage else {
        return Some("usage.json not found: can't check that the second run made no LLM call".into());
    };
    match last_generate_calls(usage) {
        Err(why) => Some(format!("usage.json is unreadable ({why}): can't check that the second run made no LLM call")),
        Ok(passes) => {
            let called: Vec<String> = passes.iter().filter(|(_, n)| *n > 0).map(|(name, n)| format!("{name}: {n}")).collect();
            let total: u64 = passes.iter().map(|(_, n)| n).sum();
            (total > 0).then(|| format!("second run made {total} LLM call(s) ({}), it should reuse everything", called.join(", ")))
        }
    }
}

/// The smoke test is conclusive when every returned list is empty. `usage` is the content of the
/// `.retrodoc/cache/usage.json` of the target, `None` when it does not exist.
fn evaluate(run1: &str, run2: &str, report: &str, usage: Option<&str>) -> Vec<String> {
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
    problems.extend(rerun_calls_problem(usage));
    for (name, log) in [("first", run1), ("second", run2)] {
        let warnings: Vec<String> =
            log.lines().map(strip_ansi).filter(|l| l.contains(" WARN ")).collect();
        if !warnings.is_empty() {
            problems.push(format!("{} warning(s) in the {name} run, e.g. {}", warnings.len(), warnings[0].trim()));
        }
    }
    problems
}

fn verdict(problems: &[String]) -> i32 {
    if problems.is_empty() {
        println!("SMOKE TEST CONCLUSIVE: generate finished, no warning, rerun is a no-op and calls no LLM, report produced");
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
    let usage = fs::read_to_string(clone.join(".retrodoc/cache/usage.json")).ok();
    let mut problems = evaluate(&read(&log1), &read(&log2), &read(&rep), usage.as_deref());
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
        Some("evaluate") if args.len() == 5 => {
            let read = |i: usize| fs::read_to_string(&args[i]).unwrap_or_else(|e| die(&format!("{}: {e}", args[i])));
            // A missing usage.json is a verdict (a failure), not a usage error.
            let usage = fs::read_to_string(&args[4]).ok();
            verdict(&evaluate(&read(1), &read(2), &read(3), usage.as_deref()))
        }
        Some("run") => run(&args[1..]),
        _ => die("usage: smoke.rs run <repo> | smoke.rs evaluate <run1> <run2> <report> <usage.json>"),
    };
    std::process::exit(code);
}
