#!/usr/bin/env rust-script
//! Behavior test for the Rust hooks: runs them against a throwaway Cargo project.
//! Usage: `just test-harness` (or `rust-script .claude/hooks/test_hooks.rs`).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Outcome {
    code: i32,
    stderr: String,
}

fn run_hook(script: &Path, project: &Path, stdin: &str) -> Outcome {
    let mut child = Command::new("rust-script")
        .arg(script)
        .env("CLAUDE_PROJECT_DIR", project)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rust-script must be installed");
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    Outcome {
        code: out.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn sh(project: &Path, program: &str, args: &[&str]) {
    let ok = Command::new(program)
        .args(args)
        .current_dir(project)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(ok, "{program} {args:?} failed");
}

fn commit_all(project: &Path, msg: &str) {
    sh(project, "git", &["add", "-A"]);
    sh(project, "git", &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", msg]);
}

fn main() {
    let here = fs::canonicalize(Path::new(file!()).parent().unwrap_or(Path::new("."))).unwrap();
    let format_hook = here.join("format_rust.rs");
    let gate = here.join("clippy_gate.rs");

    let project: PathBuf = std::env::temp_dir().join(format!("hooktest-{}", std::process::id()));
    fs::create_dir_all(&project).unwrap();
    sh(&project, "git", &["init", "-q", "."]);
    sh(&project, "cargo", &["init", "-q", "--name", "hooktest", "."]);
    commit_all(&project, "init");

    let main_rs = project.join("src/main.rs");
    let mut failed = false;
    let mut check = |name: &str, ok: bool| {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    };

    // format hook: reformats an edited .rs file, ignores other files
    fs::write(&main_rs, "fn main(){let  a=1;println!(\"{a}\");}\n").unwrap();
    let json = format!("{{\"tool_input\":{{\"file_path\":\"{}\"}}}}", main_rs.display());
    check("format hook exits 0", run_hook(&format_hook, &project, &json).code == 0);
    check(
        "format hook reformatted the file",
        fs::read_to_string(&main_rs).unwrap().contains("let a = 1;"),
    );
    fs::write(&main_rs, "fn main(){let  a=1;println!(\"{a}\");}\n").unwrap();
    let json = format!("{{\"tool_input\":{{\"file_path\":\"{}/README.md\"}}}}", project.display());
    check("format hook ignores non-Rust files", run_hook(&format_hook, &project, &json).code == 0);
    check(
        "format hook left the .rs file alone for a non-Rust edit",
        fs::read_to_string(&main_rs).unwrap().contains("let  a=1;"),
    );
    fs::write(&main_rs, "fn main() {}\n").unwrap();
    commit_all(&project, "clean");

    // clippy gate: no Rust change -> pass
    check("gate passes with no Rust change", run_hook(&gate, &project, "{}").code == 0);

    // clippy gate: warning -> exit 2 with the output on stderr
    fs::write(&main_rs, "fn main() {\n    let unused = 1;\n}\n").unwrap();
    let out = run_hook(&gate, &project, "{}");
    check("gate blocks on a clippy warning", out.code == 2);
    check("gate reports the warning to the agent", out.stderr.contains("unused"));

    // clippy gate: no infinite loop when the agent already retried
    check(
        "gate lets go when stop_hook_active",
        run_hook(&gate, &project, "{\"stop_hook_active\":true}").code == 0,
    );

    // clippy gate: fixed code -> pass
    fs::write(&main_rs, "fn main() {}\n").unwrap();
    check("gate passes once fixed", run_hook(&gate, &project, "{}").code == 0);

    let _ = fs::remove_dir_all(&project);
    std::process::exit(i32::from(failed));
}
