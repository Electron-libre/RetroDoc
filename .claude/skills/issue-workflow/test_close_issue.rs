#!/usr/bin/env rust-script
//! Behavior test for `close_issue.rs`: moves tracked and untracked issues, refuses bad targets.
//! Usage: `just test-harness`.

use std::path::Path;
use std::process::Command;

fn run(script: &Path, cwd: &Path, arg: &str) -> (bool, String) {
    let out = Command::new("rust-script").arg(script).arg(arg).current_dir(cwd).output().expect("rust-script must be installed");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
}

fn main() {
    let script = std::fs::canonicalize(Path::new(file!()).parent().unwrap().join("close_issue.rs")).unwrap();
    let tmp = std::env::temp_dir().join(format!("close_issue_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("issues")).unwrap();
    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(&tmp).status().unwrap().success());
    git(&["init", "-q"]);
    for f in ["tracked.md", "untracked.md"] {
        std::fs::write(tmp.join("issues").join(f), "x").unwrap();
    }
    git(&["add", "issues/tracked.md"]);
    std::fs::write(tmp.join("PLAN.md"), "see issues/tracked.md and my_issues/tracked.md, docs/issues/tracked.md, issues/tracked.md.bak2 (`issues/tracked.md`) issues/tracked_two.md issues/done/untracked.md").unwrap();

    let mut failed = false;
    let mut check = |name: &str, ok: bool| {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    };
    let (ok, _) = run(&script, &tmp, "issues/tracked.md");
    check("tracked issue is moved", ok && tmp.join("issues/done/tracked.md").is_file() && !tmp.join("issues/tracked.md").exists());
    let plan = std::fs::read_to_string(tmp.join("PLAN.md")).unwrap();
    check("references are pointed at done/", plan == "see issues/done/tracked.md and my_issues/tracked.md, docs/issues/tracked.md, issues/done/tracked.md.bak2 (`issues/done/tracked.md`) issues/tracked_two.md issues/done/untracked.md");
    let (ok, _) = run(&script, &tmp, "issues/untracked.md");
    check("untracked issue is moved", ok && tmp.join("issues/done/untracked.md").is_file());
    std::fs::write(tmp.join("issues/done/README.md"), "x").unwrap();
    std::fs::write(tmp.join("NOTES.md"), "issues/untracked.md issues/README.md issues/other.md").unwrap();
    std::fs::write(tmp.join("issues/other.md"), "x").unwrap();
    let (ok, _) = run(&script, &tmp, "--relink");
    let notes = std::fs::read_to_string(tmp.join("NOTES.md")).unwrap();
    check("--relink repairs done issues only", ok && notes == "issues/done/untracked.md issues/README.md issues/other.md");
    std::fs::create_dir_all(tmp.join("elsewhere")).unwrap();
    std::fs::write(tmp.join("elsewhere/x.md"), "x").unwrap();
    let (ok, out) = run(&script, &tmp, "elsewhere/x.md");
    check("file outside issues/ is refused", !ok && out.contains("not an issue"));
    let (ok, _) = run(&script, &tmp, "issues/missing.md");
    check("missing file is refused", !ok);
    std::fs::write(tmp.join("issues/tracked.md"), "y").unwrap();
    let (ok, out) = run(&script, &tmp, "issues/tracked.md");
    check("existing target is not overwritten", !ok && out.contains("already exists"));

    let _ = std::fs::remove_dir_all(&tmp);
    if failed {
        std::process::exit(1);
    }
}
