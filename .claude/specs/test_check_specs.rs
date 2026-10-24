#!/usr/bin/env rust-script
//! Behavior test for `check_specs.rs`: accepts good specs, flags each violation of `specs/README.md`.
//! Usage: `just test-harness`.

use std::path::Path;
use std::process::Command;

const SPEC: &str = "---\nid: RER\nstatus: accepted\nadrs: [0005]\n---\n\n# Rerun\n\n## Intent\n\nReuse.\n\n## Requirements\n\n- **RER-1** (deterministic) A pass must reuse a saved result.\n- **RER-2** (deterministic) A run must resume.\n\n## Scenarios\n\n- (RER-1) Given a, when b, then c.\n\n## Non-goals\n\n- None.\n";
const TESTS: &str = "// spec: RER-1, RER-2\n#[test]\nfn reuses() {}\n";

fn main() {
    let script = std::fs::canonicalize(Path::new(file!()).parent().unwrap().join("check_specs.rs")).unwrap();
    let tmp = std::env::temp_dir().join(format!("check_specs_test_{}", std::process::id()));
    let mut failed = false;
    let mut case = |name: &str, spec: &str, tests: &str, others: &[(&str, &str)], expect: Option<&str>| {
        let _ = std::fs::remove_dir_all(&tmp);
        for dir in ["specs", "crates/a/src", "docs/adr", "benchmark/x"] {
            std::fs::create_dir_all(tmp.join(dir)).unwrap();
        }
        std::fs::write(tmp.join("docs/adr/0005-incremental.md"), "x").unwrap();
        std::fs::write(tmp.join("specs/README.md"), "not a spec").unwrap();
        std::fs::write(tmp.join("specs/rerun.md"), spec).unwrap();
        std::fs::write(tmp.join("crates/a/src/t.rs"), tests).unwrap();
        for (path, body) in others {
            std::fs::write(tmp.join(path), body).unwrap();
        }
        let out = Command::new("rust-script").arg(&script).arg(&tmp).output().expect("rust-script must be installed");
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let ok = match expect {
            None => out.status.success() && stdout.is_empty(),
            Some(text) => !out.status.success() && stdout.contains(text),
        };
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            println!("{stdout}");
        }
        failed |= !ok;
    };

    case("good spec with traced tests passes", SPEC, TESTS, &[], None);
    case("a draft spec needs no test", &SPEC.replace("accepted", "draft"), "", &[], None);
    case("untraced requirement of an accepted spec is flagged", SPEC, "// spec: RER-1\nfn t() {}\n", &[], Some("RER-2"));
    case("unknown id in a test is flagged", SPEC, &format!("{TESTS}// spec: RER-9\n"), &[], Some("RER-9"));
    case("bad status is flagged", &SPEC.replace("accepted", "done"), TESTS, &[], Some("status"));
    case("requirement with another prefix is flagged", &SPEC.replace("**RER-2**", "**ABC-2**"), TESTS, &[], Some("ABC-2"));
    case("duplicate requirement id is flagged", &SPEC.replace("**RER-2**", "**RER-1**"), TESTS, &[], Some("duplicate"));
    case("missing section is flagged", &SPEC.replace("## Non-goals\n\n- None.\n", ""), TESTS, &[], Some("Non-goals"));
    case("unknown adr is flagged", &SPEC.replace("[0005]", "[0099]"), TESTS, &[], Some("0099"));
    case("unknown kind is flagged", &SPEC.replace("(deterministic) A run", "(manual) A run"), TESTS, &[], Some("kind"));
    case("withdrawn requirement needs no test", &SPEC.replace("A run must resume.", "withdrawn: A run must resume."), "// spec: RER-1\nfn t() {}\n", &[], None);
    case("duplicate spec id across files is flagged", SPEC, TESTS, &[("specs/other.md", SPEC)], Some("RER"));

    case("a live requirement that mentions withdrawn is still traced", &SPEC.replace("A run must resume.", "A withdrawn run must resume."), "// spec: RER-1\nfn t() {}\n", &[], Some("RER-2"));
    case("malformed requirement id is flagged", &SPEC.replace("**RER-2**", "**rer-2**"), TESTS, &[], Some("rer-2"));
    case("citation not above a fn is flagged", SPEC, &format!("{TESTS}// spec: RER-1\nlet x = 1;\n"), &[], Some("right above"));
    case("citation above attributes and a fn passes", SPEC, "// spec: RER-1, RER-2\n#[tokio::test]\n#[ignore]\nasync fn t() {}\n", &[], None);
    case("spec without front matter does not collide with another", "# no front matter\n", TESTS, &[("specs/other.md", "# none either\n")], Some("front matter"));

    let quality = SPEC.replace("- **RER-2** (deterministic) A run must resume.", "- **RER-2** (quality) Docs must be good.\n  benchmark: benchmark/x/reference.yaml\n  metric: recall");
    case("quality requirement with an existing benchmark passes", &quality, "// spec: RER-1\nfn t() {}\n", &[("benchmark/x/reference.yaml", "x")], None);
    case("quality requirement with a missing benchmark file is flagged", &quality, "// spec: RER-1\nfn t() {}\n", &[], Some("reference.yaml"));
    case("quality lines may follow a blank line", &quality.replace("recall\n", "recall\n\n").replace("  benchmark", "\n  benchmark"), "// spec: RER-1\nfn t() {}\n", &[("benchmark/x/reference.yaml", "x")], None);
    case("quality requirement with no metric is flagged", &quality.replace("  metric: recall\n", ""), "// spec: RER-1\nfn t() {}\n", &[("benchmark/x/reference.yaml", "x")], Some("metric"));

    let _ = std::fs::remove_dir_all(&tmp);
    if failed {
        std::process::exit(1);
    }
}
