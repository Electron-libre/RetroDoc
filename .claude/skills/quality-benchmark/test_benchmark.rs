#!/usr/bin/env rust-script
//! Behavior test for the pure parts of `benchmark.rs`: reading the pinned commit of a reference,
//! patching the clone's config, finding the previous benchmark. No LLM, no network.
//! Usage: `just test-harness`.

use std::fs;
use std::path::Path;
use std::process::Command;

const REFERENCE: &str = "repository: https://github.com/example/shop.git\ncommit: \"abc123\"  # pinned\npurpose: Sells things.\ndomains: []\n";

/// What `retrodoc init` writes (`toml::to_string_pretty` of the default config), reduced: the array of
/// existing docs is spread over several lines.
const TOML: &str = "[llm]\nmodel = \"x/y\"\nbase_url = \"http://other\"\n\n[ingest]\nextra_ignore = []\nexisting_docs_paths = [\n    \"docs\",\n    \"README.md\",\n]\n\n[output]\ndocs_dir = \"docs\"\n";

fn main() {
    let dir = Path::new(file!()).parent().unwrap();
    let script = fs::canonicalize(dir.join("benchmark.rs")).unwrap();
    let tmp = std::env::temp_dir().join(format!("benchmark-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let call = |args: &[&str]| -> (bool, String) {
        let out = Command::new("rust-script")
            .arg(&script)
            .args(args)
            .output()
            .expect("rust-script must be installed");
        (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let write = |name: &str, text: &str| {
        let path = tmp.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path.to_string_lossy().into_owned()
    };

    let mut results: Vec<(&str, bool)> = Vec::new();
    let mut check = |name: &'static str, ok: bool| results.push((name, ok));

    let reference = write("reference.yaml", REFERENCE);
    let (ok, out) = call(&["reference-field", &reference, "commit"]);
    check("reads the pinned commit, without quotes or comment", ok && out.trim() == "abc123");
    let (ok, out) = call(&["reference-field", &reference, "repository"]);
    check("reads the repository", ok && out.trim() == "https://github.com/example/shop.git");
    let (ok, _) = call(&["reference-field", &reference, "nothing"]);
    check("a missing field is an error", !ok);

    let toml = write("retrodoc.toml", TOML);
    let (ok, out) = call(&["patch-config", &toml, "m1", "http://localhost:1/v1", "true"]);
    check(
        "points [llm] at the server and hides the existing docs",
        ok && out.contains("[llm]\nmodel = \"m1\"\nbase_url = \"http://localhost:1/v1\"\nreasoning_effort = \"none\"")
            && out.contains("existing_docs_paths = []\n\n[output]")
            && !out.contains("README.md")
            && !out.contains("x/y")
            && !out.contains("http://other"),
    );
    let (ok, out) = call(&["patch-config", &toml, "m1", "http://localhost:1/v1", "false"]);
    check("keeps the existing docs when shown", ok && out.contains("existing_docs_paths = [\n    \"docs\",\n    \"README.md\",\n]"));

    let root = tmp.join("benchmarks");
    for (stamp, table) in [("100", true), ("200", true), ("250", false), ("300", true)] {
        let folder = root.join(stamp);
        fs::create_dir_all(&folder).unwrap();
        if table {
            fs::write(folder.join("table.md"), "t").unwrap();
        }
    }
    let previous = |current: &str| call(&["previous", root.to_str().unwrap(), root.join(current).to_str().unwrap()]);
    let (ok, out) = previous("300");
    check("the previous benchmark skips a run that has no table", ok && out.trim().ends_with("/200"));
    let (ok, out) = previous("200");
    check("the previous benchmark is numerically the one before", ok && out.trim().ends_with("/100"));
    let (ok, _) = previous("100");
    check("the first benchmark has no previous one", !ok);

    let mut failed = false;
    for (name, ok) in results {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    }
    let _ = fs::remove_dir_all(&tmp);
    std::process::exit(i32::from(failed));
}
