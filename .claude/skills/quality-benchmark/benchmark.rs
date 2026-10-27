#!/usr/bin/env rust-script
//! Quality benchmark of `retrodoc generate` on a public repository, against a hand-written reference.
//!
//! - `benchmark.rs run <reference.yaml> [--runs N] [--local igpu|hybrid] [--model M] [--base-url U] [--timeout-secs N] [--provider deepseek]
//!   [--series hidden|shown] [--no-confidence] [--domains-from files|surface] [--writing-temperature T]`: clones the repository
//!   of the reference at its pinned commit, then for each of the two series (`hidden`: the repo's own
//!   docs left out of the input; `shown`: with them) runs `generate` and `retrodoc benchmark --judge`
//!   N times in fresh clones, and prints the table (with the change against the previous benchmark).
//!   `--local` picks the local server (default `igpu`, the Ollama iGPU instance; `hybrid`, the llama-server);
//!   `--model` and `--base-url` override its values; `--timeout-secs` sets its HTTP timeout (default 900).
//! - `benchmark.rs reference-field <reference.yaml> <key>`: a top-level scalar of the reference.
//! - `benchmark.rs patch-config <retrodoc.toml> <provider> <model> <base-url> <hide-docs: true|false> [domains-from: files|surface]`: points
//!   `[llm]` at the local server (`provider` `local`) or at the hosted API of `provider` (`deepseek`; the key
//!   comes from `DEEPSEEK_API_KEY`, in the environment or in the `.env` of the project) and, when asked,
//!   empties `existing_docs_paths`.
//! - `benchmark.rs writing-temperature <retrodoc.toml> <t>`: the config with `temperature = t` on the writing passes
//!   (brief, domains, features, use-cases, rules), to try another value than the default 0 (ADR 0031).
//! - `benchmark.rs previous <root> <current>`: the latest benchmark directory of `root` before
//!   `current` that has a table.
//!
//! Usage: `just benchmark benchmark/<repo>/reference.yaml`. A run takes minutes to hours each: start it
//! in the background. Everything it writes (clones, logs, JSON, table) stays in the runs directory (`$RETRODOC_RUNS_DIR`, default `~/retrodoc-runs`).

use std::fs::{self, File};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEEPSEEK_MODEL: &str = "deepseek-chat";
/// The local servers `--local` names: `(name, model, base URL, command that starts it)`; the first is the default.
const LOCAL_TARGETS: [(&str, &str, &str, &str); 2] = [
    ("igpu", "qwen3.6:35b-a3b", "http://localhost:11435/v1/chat/completions", "sudo systemctl start ollama-amd"),
    ("hybrid", "qwen3.6:35b-a3b-hybride", "http://127.0.0.1:11437/v1/chat/completions", "systemctl --user start llama-hybride"),
];
/// Default HTTP timeout written in `[llm]` for the local model (`--timeout-secs` overrides it): the default (two minutes) cuts its use case
/// answers on a repository the size of linkding (85 s a call on average, some over 120 s); same value as the smoke test.
const LOCAL_TIMEOUT_SECS: &str = "900";
/// `(series name, hide the repository's own docs)`.
const SERIES: [(&str, bool); 2] = [("hidden", true), ("shown", false)];

fn die(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2);
}

/// The value of the top-level `key:` of a YAML file (quotes and a trailing comment dropped).
fn reference_field(yaml: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    let line = yaml.lines().find(|l| l.starts_with(&prefix))?;
    let value = line[prefix.len()..].split(" #").next().unwrap_or("").trim();
    let value = value.trim_matches(|c| c == '"' || c == '\'');
    (!value.is_empty()).then(|| value.to_string())
}

/// `retrodoc.toml` with `[llm]` pointed at the local server (`provider` `local`, with a timeout it can meet) or at a hosted API
/// (its own endpoint, no `reasoning_effort`, the default timeout) and, when `hide_docs`, no existing docs read as input
/// (the repository's README and docs would give the answer away).
fn patch_config(toml: &str, provider: &str, model: &str, base_url: &str, timeout_secs: &str, hide_docs: bool) -> String {
    let mut kept: Vec<String> = Vec::new();
    // Lines of an `existing_docs_paths` array being dropped (it can span several lines).
    let mut dropping = false;
    // Only the top-level keys of `[llm]` are replaced: the per-pass overrides after it are kept.
    let mut in_llm = false;
    for line in toml.lines() {
        if dropping {
            dropping = !line.trim_start().starts_with(']');
            continue;
        }
        if line.starts_with('[') {
            in_llm = line == "[llm]";
        }
        if in_llm && ["provider =", "model =", "base_url =", "reasoning_effort =", "timeout_secs ="].iter().any(|k| line.starts_with(k)) {
            continue;
        }
        if hide_docs && line.starts_with("existing_docs_paths =") {
            kept.push("existing_docs_paths = []".to_string());
            dropping = !line.contains(']');
        } else {
            kept.push(line.to_string());
        }
    }
    let llm = if provider == "local" {
        format!("[llm]\nmodel = \"{model}\"\nbase_url = \"{base_url}\"\nreasoning_effort = \"none\"\ntimeout_secs = {timeout_secs}")
    } else {
        format!("[llm]\nprovider = \"{provider}\"\nmodel = \"{model}\"")
    };
    kept.join("\n").replacen("[llm]", &llm, 1)
}

/// `toml` with `[pipeline] domains_from` set to `mode` (ADR 0020; `init` writes `files`).
fn set_domains_from(toml: &str, mode: &str) -> String {
    toml.replacen("domains_from = \"files\"", &format!("domains_from = \"{mode}\""), 1)
}

/// The passes that write prose, whose temperature the benchmark can change (default 0, ADR 0031).
const WRITING_PASSES: [&str; 5] = ["brief", "domains", "features", "use-cases", "rules"];

/// `toml` with `temperature` set on each writing pass (`[llm.passes.<name>]` sections added at the end).
fn set_writing_temperature(toml: &str, temperature: &str) -> String {
    assert!(!toml.contains("[llm.passes."), "the config already has [llm.passes.*] sections");
    let mut out = toml.trim_end().to_string();
    for pass in WRITING_PASSES {
        out.push_str(&format!("\n\n[llm.passes.{pass}]\ntemperature = {temperature}"));
    }
    out.push('\n');
    out
}

/// The benchmark directory of `root` that comes just before `current` (names are seconds since the
/// epoch) and has a `table.md`.
fn previous(root: &Path, current: &Path) -> Option<PathBuf> {
    let current_name = current.file_name()?.to_string_lossy().parse::<u64>().ok()?;
    fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| Some((e.file_name().to_string_lossy().parse::<u64>().ok()?, e.path())))
        .filter(|(stamp, path)| *stamp < current_name && path.join("table.md").exists())
        .max_by_key(|(stamp, _)| *stamp)
        .map(|(_, path)| path)
}

fn project_root() -> PathBuf {
    fs::canonicalize(Path::new(file!()).parent().unwrap().join("../../..")).unwrap()
}

fn git(args: &[&str], dir: Option<&Path>) -> bool {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    command.status().is_ok_and(|s| s.success())
}

/// The value of the flag `name` in `args`, or `default`.
fn option(args: &[String], name: &str, default: &str) -> String {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or(default.into())
}

/// `(provider, model, base URL, how to start the server)` of the LLM the run targets: the hosted
/// `--provider`, or the `--local` server; `--model` and `--base-url` override them. No start hint for
/// a hosted provider, nor for an explicit `--base-url`: it may be another server.
fn llm_target(args: &[String]) -> (String, String, String, Option<&'static str>) {
    let provider = option(args, "--provider", "local");
    if provider != "local" && provider != "deepseek" {
        die("--provider must be `local` (default) or `deepseek`");
    }
    // A `--local` with no value is an error, not the default.
    let named = args.iter().position(|a| a == "--local").map(|i| {
        args.get(i + 1).filter(|v| !v.starts_with("--")).cloned().unwrap_or_else(|| die("--local must be `igpu` or `hybrid`"))
    });
    if provider != "local" {
        if named.is_some() {
            die("--local only goes with the local provider");
        }
        return (provider, option(args, "--model", DEEPSEEK_MODEL), option(args, "--base-url", LOCAL_TARGETS[0].2), None);
    }
    let local = named.map_or(LOCAL_TARGETS[0], |named| {
        *LOCAL_TARGETS.iter().find(|t| t.0 == named).unwrap_or_else(|| die("--local must be `igpu` or `hybrid`"))
    });
    let hint = (!args.iter().any(|a| a == "--base-url")).then_some(local.3);
    (provider, option(args, "--model", local.1), option(args, "--base-url", local.2), hint)
}

fn run(args: &[String]) -> i32 {
    let Some(reference) = args.first().map(PathBuf::from) else {
        die("usage: benchmark.rs run <reference.yaml> [--runs N] [--local igpu|hybrid] [--model M] [--base-url U]")
    };
    let option = |name: &str, default: &str| option(args, name, default);
    let (provider, model, base_url, start) = llm_target(args);
    let hosted = provider != "local";
    let only_series = args.iter().position(|a| a == "--series").and_then(|i| args.get(i + 1)).cloned();
    if only_series.as_deref().is_some_and(|s| !SERIES.iter().any(|(name, _)| *name == s)) {
        die("--series must be `hidden` or `shown`");
    }
    let no_confidence = args.iter().any(|a| a == "--no-confidence");
    let domains_from = option("--domains-from", "files");
    if !["files", "surface"].contains(&domains_from.as_str()) {
        die("--domains-from must be `files` or `surface`");
    }
    let writing_temperature = option("--writing-temperature", "");
    // A TOML float: digits, a dot only between digits.
    let is_toml_float = |t: &str| {
        t.split_once('.').map_or(t, |(a, b)| if b.is_empty() { "" } else { a }).chars().all(|c| c.is_ascii_digit())
            && !t.is_empty()
            && t.split_once('.').is_none_or(|(_, b)| b.chars().all(|c| c.is_ascii_digit()))
    };
    if !writing_temperature.is_empty() && !is_toml_float(&writing_temperature) {
        die("--writing-temperature must be a number like 0 or 0.7");
    }
    let timeout_secs = option("--timeout-secs", LOCAL_TIMEOUT_SECS);
    if timeout_secs.parse::<u32>().map_or(true, |t| t == 0) {
        die("--timeout-secs must be a positive number of seconds");
    }
    let runs: u32 = option("--runs", "1").parse().unwrap_or_else(|_| die("--runs must be a number"));
    if runs == 0 {
        die("--runs must be at least 1");
    }

    let yaml = fs::read_to_string(&reference).unwrap_or_else(|e| die(&format!("{}: {e}", reference.display())));
    let field = |key: &str| reference_field(&yaml, key).unwrap_or_else(|| die(&format!("`{key}` missing from the reference")));
    let (repository, commit) = (field("repository"), field("commit"));
    let name = reference
        .canonicalize()
        .ok()
        .and_then(|p| p.parent().and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned())))
        .unwrap_or_else(|| die("the reference must live in benchmark/<repo>/"));

    // The local LLM must answer before we spend minutes building.
    let host = base_url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or("");
    let reachable = || {
        host.to_socket_addrs().ok().and_then(|mut a| {
            a.find_map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(3)).ok())
        })
    };
    if !hosted && reachable().is_none() {
        let start = start.map_or_else(String::new, |command| format!(" (`{command}`)"));
        die(&format!("no LLM server answers on {host}; start it{start} or pass --base-url"));
    }

    let root = runs_dir().join(format!("retrodoc-benchmark-{name}"));
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();
    let out = root.join(&stamp);
    let source = root.join("source");
    fs::create_dir_all(&out).unwrap();
    let fetched = if source.exists() {
        git(&["fetch", "-q", "origin"], Some(&source))
    } else {
        git(&["clone", "-q", &repository, source.to_str().unwrap()], None)
    };
    if !fetched || !git(&["checkout", "-q", &commit], Some(&source)) {
        die(&format!("could not get commit {commit} of {repository}"));
    }

    let project = project_root();
    let retrodoc = |extra: &[&str], clone: &Path, log: &Path| -> bool {
        let log = File::options().create(true).append(true).open(log).unwrap();
        Command::new("cargo")
            .args(["run", "-q", "-p", "retrodoc-cli", "--"])
            .args(extra)
            .arg("--path")
            .arg(clone)
            .current_dir(&project)
            .env("OPENROUTER_API_KEY", std::env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| "unused".into()))
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .status()
            .is_ok_and(|s| s.success())
    };

    println!("benchmark of {name} at {commit}: results in {}", out.display());
    let mut failures = 0;
    for (series, hide_docs) in SERIES {
        if only_series.as_deref().is_some_and(|only| only != series) {
            continue;
        }
        fs::create_dir_all(out.join(series)).unwrap();
        for n in 1..=runs {
            let clone = root.join(format!("clones-{stamp}")).join(format!("{series}-{n}"));
            let log = out.join(series).join(format!("run-{n}.log"));
            let json = out.join(series).join(format!("run-{n}.json"));
            println!("{series} {n}/{runs}: generate…");
            let cloned = git(&["clone", "-q", source.to_str().unwrap(), clone.to_str().unwrap()], None)
                && git(&["checkout", "-q", &commit], Some(&clone))
                && retrodoc(&["init"], &clone, &log);
            if !cloned {
                println!("FAIL - {series} {n}: could not prepare the clone (see {})", log.display());
                failures += 1;
                continue;
            }
            let toml_path = clone.join("retrodoc.toml");
            let toml = fs::read_to_string(&toml_path).unwrap();
            let patched = patch_config(&toml, &provider, &model, &base_url, &timeout_secs, hide_docs);
            let patched = set_domains_from(&patched, &domains_from);
            let patched = if writing_temperature.is_empty() { patched } else { set_writing_temperature(&patched, &writing_temperature) };
            fs::write(&toml_path, patched).unwrap();
            let generate: &[&str] = if no_confidence { &["generate", "--no-confidence"] } else { &["generate"] };
            if !retrodoc(generate, &clone, &log) {
                println!("FAIL - {series} {n}: generate failed (see {})", log.display());
                failures += 1;
                continue;
            }
            println!("{series} {n}/{runs}: judge…");
            let measured = retrodoc(
                &["benchmark", "--judge", "--reference", reference.to_str().unwrap(), "--out", json.to_str().unwrap()],
                &clone,
                &log,
            );
            if !measured {
                println!("FAIL - {series} {n}: benchmark failed (see {})", log.display());
                failures += 1;
            }
        }
    }

    let mut table_args = vec!["benchmark-table".to_string(), out.to_string_lossy().into_owned()];
    if let Some(before) = previous(&root, &out) {
        table_args.push("--previous".into());
        table_args.push(before.to_string_lossy().into_owned());
    }
    let table = Command::new("cargo")
        .args(["run", "-q", "-p", "retrodoc-cli", "--"])
        .args(&table_args)
        .current_dir(&project)
        .output()
        .unwrap();
    if table.status.success() {
        fs::write(out.join("table.md"), &table.stdout).unwrap();
        print!("\n{}", String::from_utf8_lossy(&table.stdout));
    } else {
        println!("FAIL - no table: {}", String::from_utf8_lossy(&table.stderr).trim());
        failures += 1;
    }
    i32::from(failures > 0)
}

/// Where runs are kept: `$RETRODOC_RUNS_DIR`, else `~/retrodoc-runs` (not /tmp, which is wiped at reboot).
fn runs_dir() -> PathBuf {
    let dir = std::env::var_os("RETRODOC_RUNS_DIR").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").expect("HOME is set")).join("retrodoc-runs")
    });
    fs::create_dir_all(&dir).expect("create the runs directory");
    dir
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        // Only for the test of `--local`: `llm-target [flags]` prints the model, the URL and the start command.
        Some("llm-target") => {
            let (_, model, base_url, start) = llm_target(&args[1..]);
            println!("{model}\n{base_url}\n{}", start.unwrap_or(""));
            0
        }
        Some("reference-field") if args.len() == 3 => {
            let yaml = fs::read_to_string(&args[1]).unwrap_or_else(|e| die(&format!("{}: {e}", args[1])));
            match reference_field(&yaml, &args[2]) {
                Some(value) => {
                    println!("{value}");
                    0
                }
                None => 1,
            }
        }
        Some("patch-config") if args.len() == 6 || args.len() == 7 => {
            let toml = fs::read_to_string(&args[1]).unwrap_or_else(|e| die(&format!("{}: {e}", args[1])));
            let patched = patch_config(&toml, &args[2], &args[3], &args[4], LOCAL_TIMEOUT_SECS, args[5] == "true");
            print!("{}", args.get(6).map_or(patched.clone(), |mode| set_domains_from(&patched, mode)));
            0
        }
        Some("writing-temperature") if args.len() == 3 => {
            let toml = fs::read_to_string(&args[1]).unwrap_or_else(|e| die(&format!("{}: {e}", args[1])));
            if !args[2].chars().all(|c| c.is_ascii_digit() || c == '.') || args[2].is_empty() {
                die("the temperature must be a number like 0 or 0.7");
            }
            print!("{}", set_writing_temperature(&toml, &args[2]));
            0
        }
        Some("previous") if args.len() == 3 => match previous(Path::new(&args[1]), Path::new(&args[2])) {
            Some(path) => {
                println!("{}", path.display());
                0
            }
            None => 1,
        },
        _ => die("usage: benchmark.rs run <reference.yaml> [--runs N] | reference-field <file> <key> | patch-config <toml> <provider> <model> <url> <hide> | previous <root> <current>"),
    };
    std::process::exit(code);
}
