//! Logs written to a redirected output (a file, a pipe) must be plain text: the smoke test and anyone
//! reading a saved log should not see ANSI escape codes around the level.

use std::process::Command;

/// Runs `retrodoc roles` against a closed port: the provider logs a `WARN` for each network retry,
/// without needing an LLM server.
#[test]
fn redirected_logs_have_no_ansi_codes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
    for args in [
        &["init", "-q"][..],
        &["add", "."],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "init",
        ],
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(status.success());
    }
    let retrodoc = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_retrodoc"))
            .args(args)
            .arg("--path")
            .arg(dir.path())
            .env("OPENROUTER_API_KEY", "unused")
            .output()
            .unwrap()
    };

    assert!(retrodoc(&["init"]).status.success());
    let config = dir.path().join("retrodoc.toml");
    let toml = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        toml.replacen(
            "[llm]",
            "[llm]\nbase_url = \"http://127.0.0.1:9/v1/chat/completions\"",
            1,
        ),
    )
    .unwrap();

    // `tracing` logs to stdout by default; read both streams so the test doesn't depend on that.
    let output = retrodoc(&["roles"]);
    let logs = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        logs.contains("WARN"),
        "expected a retry warning, got: {logs}"
    );
    assert!(!logs.contains('\x1b'), "logs contain ANSI codes: {logs:?}");
}
