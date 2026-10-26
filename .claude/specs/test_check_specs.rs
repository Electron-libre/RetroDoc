#!/usr/bin/env rust-script
//! Behavior test for `check_specs.rs`: accepts good specs, flags each violation of `specs/README.md`.
//! Usage: `just test-harness`.

use std::cell::Cell;
use std::path::Path;
use std::process::Command;

const SPEC: &str = "---\nid: RER\nstatus: accepted\nadrs: [0005]\n---\n\n# Rerun\n\n## Intent\n\nReuse.\n\n## Requirements\n\n- **RER-1** (deterministic) A pass must reuse a saved result.\n- **RER-2** (deterministic) A run must resume.\n\n## Scenarios\n\n- (RER-1) Given a, when b, then c.\n\n## Non-goals\n\n- None.\n";
const TESTS: &str = "// spec: RER-1, RER-2\n#[test]\nfn reuses() {}\n";

fn main() {
    let script = std::fs::canonicalize(Path::new(file!()).parent().unwrap().join("check_specs.rs")).unwrap();
    let tmp = std::env::temp_dir().join(format!("check_specs_test_{}", std::process::id()));
    let failed = Cell::new(false);
    let report = |name: &str, ok: bool| {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed.set(failed.get() | !ok);
    };
    let strict = Cell::new(false);
    let case = |name: &str, spec: &str, tests: &str, others: &[(&str, &str)], expect: Option<&str>| {
        let _ = std::fs::remove_dir_all(&tmp);
        for dir in ["specs", "crates/a/src", ".claude/hooks", "docs/adr"] {
            std::fs::create_dir_all(tmp.join(dir)).unwrap();
        }
        std::fs::write(tmp.join("docs/adr/0005-incremental.md"), "x").unwrap();
        std::fs::write(tmp.join("specs/README.md"), "not a spec").unwrap();
        std::fs::write(tmp.join("specs/rerun.md"), spec).unwrap();
        std::fs::write(tmp.join("crates/a/src/t.rs"), tests).unwrap();
        for (path, body) in others {
            std::fs::create_dir_all(tmp.join(path).parent().unwrap()).unwrap();
            std::fs::write(tmp.join(path), body).unwrap();
        }
        let mut command = Command::new("rust-script");
        command.arg(&script).arg(&tmp);
        if strict.get() {
            command.arg("--strict");
        }
        let out = command.output().expect("rust-script must be installed");
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        let ok = match expect {
            None => out.status.success() && stdout.is_empty(),
            Some(text) => !out.status.success() && stdout.contains(text),
        };
        report(name, ok);
        if !ok {
            println!("{stdout}{stderr}");
        }
        stderr
    };

    case("good spec with traced tests passes", SPEC, TESTS, &[], None);
    case("a draft spec needs no test", &SPEC.replace("accepted", "draft"), "", &[], None);
    case("untraced requirement of an accepted spec is flagged", SPEC, "// spec: RER-1\nfn t() {}\n", &[], Some("RER-2"));
    case("unknown id in a test is flagged", SPEC, &format!("{TESTS}// spec: RER-9\n"), &[], Some("RER-9"));
    case("a managed test citing a spec the project lacks passes", SPEC, TESTS, &[(".claude/hooks/test_h.rs", "// spec: PAR-1\nfn t() {}\n")], None);
    case("a managed test citing an unknown id of a present spec is flagged", SPEC, TESTS, &[(".claude/hooks/test_h.rs", "// spec: RER-9\nfn t() {}\n")], Some("RER-9"));
    case("a crate test citing a spec the project lacks is flagged", SPEC, &format!("{TESTS}// spec: PAR-1\nfn t() {{}}\n"), &[], Some("PAR-1"));
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

    case("a test under a module directory named worktrees is traced", SPEC, "// spec: RER-1\nfn t() {}\n", &[("crates/a/src/worktrees/t.rs", "// spec: RER-2\nfn t() {}\n")], None);
    case("a test under tests/target is traced", SPEC, "// spec: RER-1\nfn t() {}\n", &[("crates/a/tests/target/t.rs", "// spec: RER-2\nfn t() {}\n")], None);
    case("a citation under src/worktrees is checked", SPEC, TESTS, &[("crates/a/src/worktrees/t.rs", "// spec: RER-9\nfn t() {}\n")], Some("RER-9"));
    case("agent worktrees under .claude/worktrees are skipped", SPEC, TESTS, &[(".claude/worktrees/x/crates/a/t.rs", "// spec: RER-9\nfn t() {}\n")], None);

    case("a test of a harness script under .claude traces a requirement", SPEC, "", &[(".claude/hooks/test_h.rs", TESTS)], None);
    case("a citation under .claude not above a fn is flagged", SPEC, TESTS, &[(".claude/hooks/test_h.rs", "// spec: RER-1\nlet x = 1;\n")], Some("right above"));

    let quality = SPEC.replace("(deterministic) A run must resume.", "(quality) Docs must be good.");
    let stderr = case("a quality requirement needs no test", &quality, "// spec: RER-1\nfn t() {}\n", &[], None);
    report("a quality requirement is warned of", stderr.contains("RER-2: kind `quality` is deprecated"));
    let withdrawn = quality.replace("Docs must", "withdrawn: Docs must");
    let stderr = case("a withdrawn quality requirement passes", &withdrawn, "// spec: RER-1\nfn t() {}\n", &[], None);
    report("a withdrawn quality requirement is not warned of", !stderr.contains("kind `quality` is deprecated"));

    let implemented = SPEC.replace("status: accepted", "status: implemented");
    case("an implemented spec is enforced like accepted", &implemented, "// spec: RER-1\nfn t() {}\n", &[], Some("RER-2"));
    let stderr = case("an implemented spec passes as accepted", &implemented, TESTS, &[], None);
    report("an implemented spec is warned of", stderr.contains("warning: specs/rerun.md: status `implemented` is deprecated, use `accepted`"));

    strict.set(true);
    case("strict: a managed test citing a spec the project lacks is flagged", SPEC, TESTS, &[(".claude/hooks/test_h.rs", "// spec: PAR-1\nfn t() {}\n")], Some("PAR-1"));
    case("strict: a managed test citing a defined requirement passes", SPEC, TESTS, &[(".claude/hooks/test_h.rs", "// spec: RER-1\nfn t() {}\n")], None);

    case("strict: an unknown id of a present spec is flagged", SPEC, TESTS, &[(".claude/hooks/test_h.rs", "// spec: RER-9\nfn t() {}\n")], Some("RER-9"));
    strict.set(false);

    let bad = Command::new("rust-script").arg(&script).arg("--strcit").arg(&tmp.join("missing")).output().expect("rust-script must be installed");
    let ok = bad.status.code() == Some(2);
    report("an unknown option is rejected", ok);
    // the tree of the last case still holds the unknown id RER-9 under `.claude/`
    let first = Command::new("rust-script").arg(&script).arg("--strict").arg(&tmp).output().expect("rust-script must be installed");
    let ok = !first.status.success() && String::from_utf8_lossy(&first.stdout).contains("RER-9");
    report("--strict may come before the root", ok);

    let _ = std::fs::remove_dir_all(&tmp);
    if failed.get() {
        std::process::exit(1);
    }
}
