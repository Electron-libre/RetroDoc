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
const TOML: &str = "[llm]\nmodel = \"x/y\"\nbase_url = \"http://other\"\ntimeout_secs = 30\n\n[ingest]\nextra_ignore = []\nexisting_docs_paths = [\n    \"docs\",\n    \"README.md\",\n]\n\n[output]\ndocs_dir = \"docs\"\n";

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
    let (ok, out) = call(&["patch-config", &toml, "local", "m1", "http://localhost:1/v1", "true"]);
    check(
        "points [llm] at the server and hides the existing docs",
        ok && out.contains("[llm]\nmodel = \"m1\"\nbase_url = \"http://localhost:1/v1\"\nreasoning_effort = \"none\"")
            && out.contains("existing_docs_paths = []\n\n[output]")
            && !out.contains("README.md")
            && !out.contains("x/y")
            && !out.contains("http://other"),
    );
    check("leaves the timeout to the default of a local server", ok && !out.contains("timeout_secs"));
    let (ok, out) = call(&["patch-config", &toml, "local", "m1", "http://localhost:1/v1", "true", "", "60"]);
    check(
        "writes the one timeout --timeout-secs gives",
        ok && out.contains("timeout_secs = 60\n") && out.matches("timeout_secs").count() == 1,
    );
    let (ok, out) = call(&["patch-config", &toml, "local", "m1", "http://localhost:1/v1", "false"]);
    check("keeps the existing docs when shown", ok && out.contains("existing_docs_paths = [\n    \"docs\",\n    \"README.md\",\n]"));

    let with_pass = write("with-pass.toml", &format!("{TOML}\n[llm.passes.use-cases]\nmodel = \"keep/me\"\ntimeout_secs = 1800\n"));
    let (ok, out) = call(&["patch-config", &with_pass, "local", "m1", "http://localhost:1/v1", "false"]);
    check(
        "keeps the keys of a per-pass override",
        ok && out.contains("[llm.passes.use-cases]\nmodel = \"keep/me\"\ntimeout_secs = 1800") && out.matches("timeout_secs").count() == 1,
    );

    let (ok, out) = call(&["patch-config", &toml, "deepseek", "deepseek-chat", "-", "false"]);
    check(
        "points [llm] at a hosted provider without base_url or reasoning_effort",
        ok && out.contains("[llm]\nprovider = \"deepseek\"\nmodel = \"deepseek-chat\"")
            && !out.contains("base_url")
            && !out.contains("reasoning_effort")
            && !out.contains("timeout_secs"),
    );

    let with_pipeline = write("with-pipeline.toml", &format!("{TOML}\n[pipeline]\ndomains_from = \"files\"\n"));
    let (ok, out) = call(&["patch-config", &with_pipeline, "local", "m1", "http://localhost:1/v1", "false", "surface"]);
    check("sets domains_from when asked", ok && out.contains("domains_from = \"surface\"") && !out.contains("\"files\""));
    let (ok, out) = call(&["patch-config", &with_pipeline, "local", "m1", "http://localhost:1/v1", "false"]);
    check("leaves domains_from alone by default", ok && out.contains("domains_from = \"files\""));
    let with_rules = write("with-rules.toml", &format!("{TOML}\n[pipeline]\nrules = true\n"));
    let (ok, out) = call(&["patch-config", &with_rules, "local", "m1", "http://localhost:1/v1", "false"]);
    check("turns the rules pass off (the benchmark does not judge the rules)", ok && out.contains("rules = false") && !out.contains("rules = true"));
    let (ok, out) = call(&["patch-config", &with_pipeline, "local", "m1", "http://localhost:1/v1", "false"]);
    check("turns the rules pass off when the key is absent", ok && out.contains("[pipeline]\nrules = false\n"));
    let with_tables = write("with-tables.toml", &format!("{TOML}\n[pipeline]\ndomains_from = \"files\"\n[[x]]\nrules = true\n"));
    let (ok, out) = call(&["patch-config", &with_tables, "local", "m1", "http://localhost:1/v1", "false"]);
    check("stops the [pipeline] section at an array of tables", ok && out.contains("[pipeline]\nrules = false") && out.contains("[[x]]\nrules = true"));
    let with_array = write("with-array.toml", &format!("{TOML}\n[pipeline]\nx = [\n  [\"a\"],\n]\nrules = true\n"));
    let (ok, out) = call(&["patch-config", &with_array, "local", "m1", "http://localhost:1/v1", "false"]);
    check("turns the rules pass off past an array value", ok && out.matches("rules =").count() == 1 && out.contains("rules = false"));
    let (ok, out) = call(&["patch-config", &toml, "local", "m1", "http://localhost:1/v1", "false"]);
    check("turns the rules pass off without a [pipeline] section", ok && out.contains("[pipeline]\nrules = false"));

    let (ok, out) = call(&["writing-temperature", &toml, "0.7"]);
    check(
        "sets the temperature of the writing passes only",
        ok && ["brief", "domains", "features", "use-cases", "rules"]
            .iter()
            .all(|p| out.contains(&format!("[llm.passes.{p}]\ntemperature = 0.7")))
            && !out.contains("[llm.passes.roles]")
            && out.contains("README.md"),
    );

    // `--local` names a local server; explicit flags win; a hosted provider takes no `--local`.
    let (ok, out) = call(&["llm-target"]);
    check("the default local target is the Ollama iGPU instance", ok && out.starts_with("qwen3.6:35b-a3b\nhttp://localhost:11435/v1/chat/completions\n"));
    let (ok, out) = call(&["llm-target", "--local", "hybrid"]);
    check(
        "--local hybrid targets the llama-server and names its service",
        ok && out.starts_with("qwen3.6:35b-a3b-hybride\nhttp://127.0.0.1:11437/v1/chat/completions\n")
            && out.contains("systemctl --user start llama-hybride"),
    );
    let (ok, out) = call(&["llm-target", "--local", "hybrid", "--model", "m", "--base-url", "u"]);
    check("--model and --base-url win over --local, and name no service", ok && out == "m\nu\n\n");
    let (ok, _) = call(&["llm-target", "--local"]);
    check("--local without a value is an error", !ok);
    let (ok, _) = call(&["llm-target", "--local", "nope"]);
    check("an unknown --local is an error", !ok);
    let (ok, out) = call(&["llm-target", "--provider", "deepseek"]);
    check("the hosted provider keeps its model", ok && out.starts_with("deepseek-chat\n"));
    let (ok, _) = call(&["llm-target", "--provider", "deepseek", "--local", "hybrid"]);
    check("--local with a hosted provider is an error", !ok);

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

    let skill = fs::read_to_string(dir.join("SKILL.md")).unwrap();
    check(
        "the skill reads and appends the ledger of measures",
        skill.contains("~/retrodoc-runs/LEDGER.md") && skill.contains("## Ledger of measures") && skill.contains("**Before**") && skill.contains("**After**"),
    );

    let mut failed = false;
    for (name, ok) in results {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    }
    let _ = fs::remove_dir_all(&tmp);
    std::process::exit(i32::from(failed));
}
