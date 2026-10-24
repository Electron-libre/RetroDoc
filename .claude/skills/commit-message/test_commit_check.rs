#!/usr/bin/env rust-script
//! Behavior test for `commit_check.rs`: good messages pass, each rule of `AGENTS.md` is enforced.
//! Usage: `just test-harness`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn check(script: &Path, message: &str, cwd: &Path) -> (bool, String) {
    let mut child = Command::new("rust-script")
        .arg(script)
        .current_dir(cwd)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rust-script must be installed");
    child.stdin.take().unwrap().write_all(message.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

/// Make `dir` a git repository, the root the script resolves the config from.
fn git_init(dir: &Path) {
    let status = Command::new("git").arg("init").arg("-q").arg(dir).status().expect("git must be installed");
    assert!(status.success());
}

fn main() {
    let script = std::fs::canonicalize(Path::new(file!()).parent().unwrap().join("commit_check.rs")).unwrap();
    // Projects whose harness.toml holds a look-alike key before the real one, or comments inside the list.
    let project = |name: &str, toml: &str| {
        let dir = std::env::temp_dir().join(format!("commit_check_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        std::fs::write(dir.join(".claude/harness.toml"), toml).unwrap();
        git_init(&dir);
        dir
    };
    let lookalike = project("lookalike", "[project]\ncommit_scopes_extra = [\"wrong\"]\ncommit_scopes = [\"right\"]\n");
    let commented = project("commented", "[project]\ncommit_scopes = [\n  \"a\", # a \"quote\" and a ]\n  \"b\",\n]\n");
    let nested = project("nested", "[project]\ncommit_scopes = [\"deep\"]\n");
    std::fs::create_dir_all(nested.join("a/b")).unwrap();
    let nested_cwd = nested.join("a/b");
    let missing = project("missing", "");
    std::fs::remove_file(missing.join(".claude/harness.toml")).unwrap();
    let invalid = project("invalid", "[project\ncommit_scopes = \n");
    let not_a_repo = std::env::temp_dir().join(format!("commit_check_{}_norepo", std::process::id()));
    let _ = std::fs::remove_dir_all(&not_a_repo);
    std::fs::create_dir_all(&not_a_repo).unwrap();
    let long_line = "word ".repeat(16);
    let here = std::env::current_dir().unwrap();
    let cases: Vec<(&str, String, bool, &str, &Path)> = vec![
        ("valid subject only", "fix(docs): retry on 429 with backoff\n".into(), true, "", &here),
        ("valid without scope", "fix: handle empty repository\n".into(), true, "", &here),
        ("valid with body and footer", "feat(harness): add progress reporting\n\nLog one line per unit.\n\nRefs: #12\n".into(), true, "", &here),
        ("mentioning Claude Code as the subject matter", "docs: describe the Claude Code hooks\n\nThe Anthropic CLI runs them.\n".into(), true, "", &here),
        ("subject of exactly 72 chars", format!("feat(docs): {}\n", "a".repeat(72 - "feat(docs): ".len())), true, "", &here),
        ("subject too long", format!("feat(docs): {}\n", "a".repeat(73 - "feat(docs): ".len())), false, "72", &here),
        ("unknown type", "feature(docs): add flag\n".into(), false, "type", &here),
        ("unknown scope", "feat(frontend): add flag\n".into(), false, "scope", &here),
        ("capitalized description", "feat(docs): Add flag\n".into(), false, "lowercase", &here),
        ("trailing period", "feat(docs): add flag.\n".into(), false, "period", &here),
        ("missing colon format", "feat(cli) add flag\n".into(), false, "format", &here),
        ("no blank line before body", "feat(docs): add flag\nbody here\n".into(), false, "blank", &here),
        ("body line too long", format!("feat(docs): add flag\n\n{long_line}\n"), false, "72", &here),
        ("co-authored-by trailer", "feat(docs): add flag\n\nCo-Authored-By: Someone <a@b.c>\n".into(), false, "attribution", &here),
        ("generated-with line", "feat(docs): add flag\n\nGenerated with Claude Code\n".into(), false, "attribution", &here),
        ("markdown heading", "feat(docs): add flag\n\n# Why\n".into(), false, "markdown", &here),
        ("code fence", "feat(docs): add flag\n\n```\nx\n```\n".into(), false, "markdown", &here),
        ("emoji", "feat(docs): add flag \u{1F680}\n".into(), false, "emoji", &here),
        ("look-alike key does not shadow the real one", "feat(right): add flag\n".into(), true, "", &lookalike),
        ("look-alike key is not read as the real one", "feat(wrong): add flag\n".into(), false, "scope", &lookalike),
        ("list with a commented quote and bracket", "feat(b): add flag\n".into(), true, "", &commented),
        ("config is read from the repository root, not the current directory", "feat(deep): add flag\n".into(), true, "", &nested_cwd),
        ("missing harness.toml is reported", "feat(docs): add flag\n".into(), false, "harness.toml", &missing),
        ("invalid harness.toml is reported", "feat(docs): add flag\n".into(), false, "harness.toml", &invalid),
        ("outside a git repository is reported", "feat(docs): add flag\n".into(), false, "git", &not_a_repo),
    ];
    let mut failed = false;
    for (name, message, should_pass, hint, cwd) in cases {
        let (passed, output) = check(&script, &message, cwd);
        let ok = passed == should_pass && (should_pass || output.contains(hint));
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            println!("       got pass={passed}, output: {}", output.trim());
        }
        failed |= !ok;
    }
    for dir in [&lookalike, &commented, &nested, &missing, &invalid, &not_a_repo] {
        let _ = std::fs::remove_dir_all(dir);
    }
    std::process::exit(i32::from(failed));
}
