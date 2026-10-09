#!/usr/bin/env rust-script
//! Behavior test for `next_issue.rs` and the queue handling of `close_issue.rs`.
//! Usage: `just test-harness`.

use std::path::Path;
use std::process::Command;

fn main() {
    let dir = std::fs::canonicalize(Path::new(file!()).parent().unwrap()).unwrap();
    let tmp = std::env::temp_dir().join(format!("next_issue_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("issues")).unwrap();
    let w = |f: &str, t: &str| std::fs::write(tmp.join("issues").join(f), t).unwrap();
    w("a.md", "# Title A\n");
    w("b.md", "# Title B\n");
    w("c.md", "# Title C\n");
    w("queue.md", "# Issue queue\n\nProse.\n\n- [b](b.md) - first\n- [a](a.md) - second\n- [ghost](ghost.md) - gone\n");
    let run = |script: &str, arg: &str| {
        let out = Command::new("rust-script").arg(dir.join(script)).arg(arg).current_dir(&tmp).output().expect("rust-script must be installed");
        (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
    };
    let mut failed = false;
    let mut check = |name: &str, ok: bool| {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    };
    let (ok, out) = run("next_issue.rs", "issues");
    check("first line is the next issue, with its title", out.contains("Next: issues/b.md") && out.contains("Title B"));
    check("line without a file is reported", !ok && out.contains("queue line without a file: ghost"));
    check("issue absent from the queue is reported", out.contains("missing from the queue: c"));
    let (_, out) = run("close_issue.rs", "issues/b.md");
    let queue = std::fs::read_to_string(tmp.join("issues/queue.md")).unwrap();
    check("closing removes the line from the queue", out.contains("removed b") && !queue.contains("[b](b.md) - first") && queue.contains("[a](a.md) - second"));
    let (_, out) = run("next_issue.rs", "issues");
    check("next moves on after closing", out.contains("Next: issues/a.md"));

    let _ = std::fs::remove_dir_all(&tmp);
    if failed {
        std::process::exit(1);
    }
}
