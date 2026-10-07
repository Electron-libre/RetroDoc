#!/usr/bin/env rust-script
//! Quality benchmark of `retrodoc generate` on a public repository, against a hand-written reference.
//!
//! - `benchmark.rs run <reference.yaml> [--runs N] [--model M] [--base-url U]`: clones the repository
//!   of the reference at its pinned commit, then for each of the two series (`hidden`: the repo's own
//!   docs left out of the input; `shown`: with them) runs `generate` and `retrodoc benchmark --judge`
//!   N times in fresh clones, and prints the table (with the change against the previous benchmark).
//! - `benchmark.rs reference-field <reference.yaml> <key>`: a top-level scalar of the reference.
//! - `benchmark.rs patch-config <retrodoc.toml> <model> <base-url> <hide-docs: true|false>`: points
//!   `[llm]` at the local server and, when asked, empties `existing_docs_paths`.
//! - `benchmark.rs previous <root> <current>`: the latest benchmark directory of `root` before
//!   `current` that has a table.
//!
//! Usage: `just benchmark benchmark/<repo>/reference.yaml`. A run takes minutes to hours each: start it
//! in the background. Everything it writes (clones, logs, JSON, table) stays in /tmp.

use std::fs::{self, File};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_MODEL: &str = "qwen3.6:35b-a3b";
const DEFAULT_BASE_URL: &str = "http://localhost:11435/v1/chat/completions";
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

/// `retrodoc.toml` with `[llm]` pointed at the local server and, when `hide_docs`, no existing docs
/// read as input (the repository's README and docs would give the answer away).
fn patch_config(toml: &str, model: &str, base_url: &str, hide_docs: bool) -> String {
    let kept: Vec<String> = toml
        .lines()
        .filter(|l| !["model =", "base_url =", "reasoning_effort ="].iter().any(|k| l.starts_with(k)))
        .map(|l| {
            if hide_docs && l.starts_with("existing_docs_paths =") {
                "existing_docs_paths = []".to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    kept.join("\n").replacen(
        "[llm]",
        &format!("[llm]\nmodel = \"{model}\"\nbase_url = \"{base_url}\"\nreasoning_effort = \"none\""),
        1,
    )
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

fn run(args: &[String]) -> i32 {
    let Some(reference) = args.first().map(PathBuf::from) else {
        die("usage: benchmark.rs run <reference.yaml> [--runs N] [--model M] [--base-url U]")
    };
    let option = |name: &str, default: &str| {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or(default.into())
    };
    let (model, base_url) = (option("--model", DEFAULT_MODEL), option("--base-url", DEFAULT_BASE_URL));
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

    // The LLM must answer before we spend minutes building.
    let host = base_url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or("");
    let reachable = host.to_socket_addrs().ok().and_then(|mut a| {
        a.find_map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(3)).ok())
    });
    if reachable.is_none() {
        die(&format!("no LLM server answers on {host}; start it or pass --base-url"));
    }

    let root = std::env::temp_dir().join(format!("retrodoc-benchmark-{name}"));
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
            .env("OPENROUTER_API_KEY", "unused")
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .status()
            .is_ok_and(|s| s.success())
    };

    println!("benchmark of {name} at {commit}: results in {}", out.display());
    let mut failures = 0;
    for (series, hide_docs) in SERIES {
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
            fs::write(&toml_path, patch_config(&toml, &model, &base_url, hide_docs)).unwrap();
            if !retrodoc(&["generate"], &clone, &log) {
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
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
        Some("patch-config") if args.len() == 5 => {
            let toml = fs::read_to_string(&args[1]).unwrap_or_else(|e| die(&format!("{}: {e}", args[1])));
            print!("{}", patch_config(&toml, &args[2], &args[3], args[4] == "true"));
            0
        }
        Some("previous") if args.len() == 3 => match previous(Path::new(&args[1]), Path::new(&args[2])) {
            Some(path) => {
                println!("{}", path.display());
                0
            }
            None => 1,
        },
        _ => die("usage: benchmark.rs run <reference.yaml> [--runs N] | reference-field <file> <key> | patch-config <toml> <model> <url> <hide> | previous <root> <current>"),
    };
    std::process::exit(code);
}
