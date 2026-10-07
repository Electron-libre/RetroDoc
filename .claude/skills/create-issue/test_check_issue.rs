#!/usr/bin/env rust-script
//! Behavior test for `check_issue.rs`: accepts a good issue, flags each template violation.
//! Usage: `just test-harness`.

use std::path::Path;
use std::process::Command;

const GOOD: &str = "# Fix the thing\n\n# Goal\n\nMake it work.\n\n# Approach\n\n1. Do it.\n\n# Resources\n\n* `a.rs`\n";

fn main() {
    let script = std::fs::canonicalize(Path::new(file!()).parent().unwrap().join("check_issue.rs")).unwrap();
    let tmp = std::env::temp_dir().join(format!("check_issue_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("issues/done")).unwrap();
    std::fs::write(tmp.join("issues/done/old.md"), "x").unwrap();

    let run = |file: &str, body: &str| {
        std::fs::write(tmp.join(file), body).unwrap();
        let out = Command::new("rust-script").arg(&script).arg(file).current_dir(&tmp).output().expect("rust-script must be installed");
        (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let mut failed = false;
    let mut check = |name: &str, ok: bool| {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    };
    let (ok, out) = run("issues/good.md", GOOD);
    check("good issue passes", ok && out.is_empty());
    let (ok, out) = run("issues/Bad-Name.md", GOOD);
    check("non snake_case name is flagged", !ok && out.contains("snake_case"));
    let (ok, out) = run("issues/old.md", GOOD);
    check("name already in done/ is flagged", !ok && out.contains("already exists"));
    let (ok, out) = run("issues/a.md", &GOOD.replace("# Approach\n\n1. Do it.\n\n", ""));
    check("missing section is flagged", !ok && out.contains("`# Approach`"));
    let (ok, out) = run("issues/b.md", &format!("{GOOD}\n# Tracking\n"));
    check("tracking section is flagged", !ok && out.contains("Tracking"));
    let (ok, out) = run("issues/c.md", &GOOD.replace("Make it work.", ""));
    check("empty goal is flagged", !ok && out.contains("empty"));
    let (ok, out) = run("issues/d.md", &GOOD.replace("Make it work.", "See /home/me/x."));
    check("private path is flagged", !ok && out.contains("private path"));
    let (ok, out) = run("issues/e.md", &GOOD.replace("# Fix the thing\n\n", ""));
    check("missing title is flagged", !ok && out.contains("title"));

    let _ = std::fs::remove_dir_all(&tmp);
    if failed {
        std::process::exit(1);
    }
}
