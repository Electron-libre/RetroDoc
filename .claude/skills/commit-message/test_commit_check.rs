#!/usr/bin/env rust-script
//! Behavior test for `commit_check.rs`: good messages pass, each rule of `AGENTS.md` is enforced.
//! Usage: `just test-harness`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn check(script: &Path, message: &str) -> (bool, String) {
    let mut child = Command::new("rust-script")
        .arg(script)
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

fn main() {
    let script = std::fs::canonicalize(Path::new(file!()).parent().unwrap().join("commit_check.rs")).unwrap();
    let long_line = "word ".repeat(16);
    let cases: Vec<(&str, String, bool, &str)> = vec![
        ("valid subject only", "fix(llm): retry on 429 with backoff\n".into(), true, ""),
        ("valid without scope", "fix: handle empty repository\n".into(), true, ""),
        ("valid with body and footer", "feat(pipeline): add progress reporting\n\nLog one line per unit.\n\nRefs: #12\n".into(), true, ""),
        ("mentioning Claude Code as the subject matter", "docs: describe the Claude Code hooks\n\nThe Anthropic CLI runs them.\n".into(), true, ""),
        ("subject of exactly 72 chars", format!("feat(cli): {}\n", "a".repeat(72 - "feat(cli): ".len())), true, ""),
        ("subject too long", format!("feat(cli): {}\n", "a".repeat(73 - "feat(cli): ".len())), false, "72"),
        ("unknown type", "feature(cli): add flag\n".into(), false, "type"),
        ("unknown scope", "feat(frontend): add flag\n".into(), false, "scope"),
        ("capitalized description", "feat(cli): Add flag\n".into(), false, "lowercase"),
        ("trailing period", "feat(cli): add flag.\n".into(), false, "period"),
        ("missing colon format", "feat(cli) add flag\n".into(), false, "format"),
        ("no blank line before body", "feat(cli): add flag\nbody here\n".into(), false, "blank"),
        ("body line too long", format!("feat(cli): add flag\n\n{long_line}\n"), false, "72"),
        ("co-authored-by trailer", "feat(cli): add flag\n\nCo-Authored-By: Someone <a@b.c>\n".into(), false, "attribution"),
        ("generated-with line", "feat(cli): add flag\n\nGenerated with Claude Code\n".into(), false, "attribution"),
        ("markdown heading", "feat(cli): add flag\n\n# Why\n".into(), false, "markdown"),
        ("code fence", "feat(cli): add flag\n\n```\nx\n```\n".into(), false, "markdown"),
        ("emoji", "feat(cli): add flag \u{1F680}\n".into(), false, "emoji"),
    ];
    let mut failed = false;
    for (name, message, should_pass, hint) in cases {
        let (passed, output) = check(&script, &message);
        let ok = passed == should_pass && (should_pass || output.contains(hint));
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            println!("       got pass={passed}, output: {}", output.trim());
        }
        failed |= !ok;
    }
    std::process::exit(i32::from(failed));
}
