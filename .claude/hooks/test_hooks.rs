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
    run_hook_with(script, project, stdin, &[])
}

fn run_hook_with(script: &Path, project: &Path, stdin: &str, env: &[(&str, String)]) -> Outcome {
    let mut child = Command::new("rust-script")
        .arg(script)
        .env("CLAUDE_PROJECT_DIR", project)
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rust-script must be installed");
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    Outcome { code: out.status.code().unwrap_or(-1), stderr: String::from_utf8_lossy(&out.stderr).into_owned() }
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

// spec: CLG-1, CLG-2, CLG-3, CLG-4, CLG-5
/// The clippy gate in one Cargo project, in order: the stamp of a passing run decides the runs that follow.
fn clippy_gate_checks(gate: &Path, project: &Path) -> Vec<(&'static str, bool)> {
    let main_rs = project.join("src/main.rs");
    let mut checks = Vec::new();
    // A fake `cargo` first on the PATH: it counts its runs, passes while `fake_pass` exists and fails with the
    // diagnostics of `fake_out` otherwise.
    let fake_bin = project.join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let fake_out = project.join("fake-out.txt");
    let fake_log = project.join("fake-log.txt");
    let fake_pass = project.join("fake-pass");
    let fake_cargo = fake_bin.join("cargo");
    fs::write(
        &fake_cargo,
        format!(
            "#!/bin/sh\necho run >> '{}'\n[ -e '{}' ] && exit 0\ncat '{}' >&2\nexit 101\n",
            fake_log.display(),
            fake_pass.display(),
            fake_out.display()
        ),
    )
    .unwrap();
    sh(project, "chmod", &["+x", fake_cargo.to_str().unwrap()]);
    let fake_env = [("PATH", format!("{}:{}", fake_bin.display(), std::env::var("PATH").unwrap_or_default()))];
    let fake_runs = || fs::read_to_string(&fake_log).map(|l| l.lines().count()).unwrap_or(0);
    fs::write(project.join(".gitignore"), "/target\n/fake-*\n").unwrap();
    commit_all(project, "ignore");

    // rust-script builds the hook with cargo: build it with the real one before the fake shadows it.
    run_hook(gate, project, "{\"stop_hook_active\":true}");
    checks.push(("gate passes with no Rust change", run_hook_with(gate, project, "{}", &fake_env).code == 0));
    checks.push(("gate runs no clippy with no Rust change", fake_runs() == 0));
    fs::create_dir_all(project.join("tools/.claude/hooks")).unwrap();
    fs::write(project.join("tools/.claude/hooks/hook.rs"), "").unwrap();
    checks.push((
        "gate runs no clippy for a changed harness script only",
        run_hook_with(gate, project, "{}", &fake_env).code == 0 && fake_runs() == 0,
    ));
    fs::remove_dir_all(project.join("tools")).unwrap();

    fs::write(&main_rs, "fn main() {\n    let unused = 1;\n}\n").unwrap();
    let out = run_hook(gate, project, "{}");
    checks.push(("gate blocks on a clippy warning", out.code == 2));
    checks.push((
        "gate reports the file, line and message",
        out.stderr.contains("src/main.rs:2:9:") && out.stderr.contains("unused"),
    ));
    checks.push((
        "gate reports no build progress",
        !out.stderr.contains("Checking")
            && !out.stderr.contains("Compiling")
            && !out.stderr.contains("could not compile"),
    ));

    checks.push((
        "gate lets go when stop_hook_active",
        run_hook_with(gate, project, "{\"stop_hook_active\":true}", &fake_env).code == 0 && fake_runs() == 0,
    ));

    let many: String = (1..=45).map(|i| format!("src/main.rs:{i}:1: error: problem {i}\n")).collect();
    fs::write(&fake_out, format!("    Checking hooktest v0.1.0\n{many}error: could not compile `hooktest`\n")).unwrap();
    let out = run_hook_with(gate, project, "{}", &fake_env);
    let reported = out.stderr.lines().filter(|l| l.starts_with("src/main.rs:")).count();
    checks.push(("gate reports at most 40 diagnostics", out.code == 2 && reported == 40));
    checks.push(("gate says how many it left out", out.stderr.contains("5 more")));

    let before = fake_runs();
    run_hook_with(gate, project, "{}", &fake_env);
    checks.push(("gate runs clippy again after a failure with nothing changed", fake_runs() == before + 1));

    fs::write(&main_rs, "fn main() {}\n// edited\n").unwrap();
    checks.push(("gate passes once fixed", run_hook(gate, project, "{}").code == 0));
    let before = fake_runs();
    fs::write(project.join("README.md"), "edited\n").unwrap();
    checks.push((
        "gate skips clippy while the Rust state is unchanged",
        run_hook_with(gate, project, "{}", &fake_env).code == 0 && fake_runs() == before,
    ));

    fs::write(&main_rs, "fn main() {}\n// edited again\n").unwrap();
    checks.push(("gate runs clippy again after a .rs edit", run_hook_with(gate, project, "{}", &fake_env).code == 2));
    fs::write(&main_rs, "fn main() {}\n// edited\n").unwrap();
    checks.push((
        "gate skips again once back to the passing state",
        run_hook_with(gate, project, "{}", &fake_env).code == 0,
    ));
    fs::write(project.join("src/extra.rs"), "").unwrap();
    checks.push((
        "gate runs clippy again for a new untracked .rs file",
        run_hook_with(gate, project, "{}", &fake_env).code == 2,
    ));
    fs::remove_file(project.join("src/extra.rs")).unwrap();
    sh(project, "git", &["add", "README.md"]);
    sh(project, "git", &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "readme"]);
    checks.push(("gate runs clippy again after HEAD moved", run_hook_with(gate, project, "{}", &fake_env).code == 2));

    fs::write(
        &fake_out,
        "   Compiling dep v1.0.0\nsrc/main.rs:1:1: error: located\nerror: failed to run custom build command for `dep`\n\
         error: could not compile `hooktest`\n",
    )
    .unwrap();
    let out = run_hook_with(gate, project, "{}", &fake_env);
    checks.push((
        "gate reports the errors that have no location",
        out.stderr.contains("src/main.rs:1:1: error: located") && out.stderr.contains("custom build command"),
    ));
    checks.push((
        "gate reports no build progress with an unlocated error",
        !out.stderr.contains("Compiling") && !out.stderr.contains("could not compile"),
    ));
    fs::write(&fake_out, "   Compiling dep v1.0.0\n    Checking hooktest\nerror: failed to parse manifest\n").unwrap();
    let out = run_hook_with(gate, project, "{}", &fake_env);
    checks.push((
        "gate reports no build progress without any located diagnostic",
        out.code == 2 && out.stderr.contains("failed to parse manifest") && !out.stderr.contains("Compiling"),
    ));

    // the stamp reads the content of the files, whatever their name or the diff settings
    fs::write(&fake_pass, "").unwrap();
    let cafe = project.join("src/café.rs");
    fs::write(&cafe, "// one\n").unwrap();
    run_hook_with(gate, project, "{}", &fake_env);
    fs::remove_file(&fake_pass).unwrap();
    fs::write(&cafe, "// two\n").unwrap();
    checks.push((
        "gate runs clippy again after an edit of a non-ASCII file",
        run_hook_with(gate, project, "{}", &fake_env).code == 2,
    ));
    fs::remove_file(&cafe).unwrap();
    fs::write(project.join(".gitattributes"), "*.rs -diff\n").unwrap();
    fs::write(&fake_pass, "").unwrap();
    run_hook_with(gate, project, "{}", &fake_env);
    fs::remove_file(&fake_pass).unwrap();
    fs::write(&main_rs, "fn main() {}\n// a -diff edit\n").unwrap();
    checks.push((
        "gate runs clippy again after an edit git shows no diff of",
        run_hook_with(gate, project, "{}", &fake_env).code == 2,
    ));

    // a repository with no commit yet: a staged file edited again
    let fresh = project.join("fake-fresh");
    fs::create_dir_all(fresh.join("src")).unwrap();
    sh(&fresh, "git", &["init", "-q", "."]);
    fs::write(fresh.join("Cargo.toml"), "").unwrap();
    fs::write(fresh.join("src/main.rs"), "fn main() {}\n").unwrap();
    sh(&fresh, "git", &["add", "-A"]);
    fs::write(&fake_pass, "").unwrap();
    run_hook_with(gate, &fresh, "{}", &fake_env);
    fs::remove_file(&fake_pass).unwrap();
    fs::write(fresh.join("src/main.rs"), "fn main() { let unused = 1; }\n").unwrap();
    checks.push((
        "gate runs clippy again after an edit of a staged file with no commit",
        run_hook_with(gate, &fresh, "{}", &fake_env).code == 2,
    ));
    fs::remove_file(fresh.join("Cargo.toml")).unwrap();
    let before = fake_runs();
    checks.push((
        "gate runs no clippy in a project without Cargo.toml",
        run_hook_with(gate, &fresh, "{}", &fake_env).code == 0 && fake_runs() == before,
    ));

    checks
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
    check("format hook reformatted the file", fs::read_to_string(&main_rs).unwrap().contains("let a = 1;"));
    fs::write(&main_rs, "fn main(){let  a=1;println!(\"{a}\");}\n").unwrap();
    let json = format!("{{\"tool_input\":{{\"file_path\":\"{}/README.md\"}}}}", project.display());
    check("format hook ignores non-Rust files", run_hook(&format_hook, &project, &json).code == 0);
    check(
        "format hook left the .rs file alone for a non-Rust edit",
        fs::read_to_string(&main_rs).unwrap().contains("let  a=1;"),
    );
    fs::write(&main_rs, "fn main() {}\n").unwrap();
    commit_all(&project, "clean");

    for (name, ok) in clippy_gate_checks(&gate, &project) {
        check(name, ok);
    }

    let _ = fs::remove_dir_all(&project);
    std::process::exit(i32::from(failed));
}
