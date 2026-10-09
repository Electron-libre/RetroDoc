//! `source-hashes.json`: written by a `generate` that went through every pass, left alone by one
//! that failed, so the MCP freshness check never trusts hashes the docs were not built from.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};

/// A chat-completions server that answers every request with `status` and `body`.
fn server(status: &'static str, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/chat/completions", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buf = vec![0u8; 65536];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    url
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=A", "-c", "user.email=a@example.com"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

fn repo(url: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("app.rb"), "class Order; end\n").unwrap();
    std::fs::write(
        dir.path().join("retrodoc.toml"),
        format!(
            "[llm]\napi_key_env = \"RETRODOC_TEST_SOURCE_HASHES_KEY\"\nmodel = \"m\"\n\
             base_url = \"{url}\"\nstructured_output = false\n"
        ),
    )
    .unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);
    dir
}

fn generate(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["generate", "--no-confidence", "--path"])
        .arg(dir)
        .env("RETRODOC_TEST_SOURCE_HASHES_KEY", "unused")
        .output()
        .unwrap()
}

fn saved(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(".retrodoc/cache/source-hashes.json")).ok()
}

#[test]
fn a_failed_run_leaves_the_hashes_of_the_last_complete_run() {
    let dir = repo(&server("400 Bad Request", r#"{"error":"no"}"#));
    let cache = dir.path().join(".retrodoc/cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join("source-hashes.json"),
        r#"{"files":{"app.rb":"old"}}"#,
    )
    .unwrap();

    let output = generate(dir.path());

    assert!(!output.status.success());
    assert_eq!(
        saved(dir.path()).as_deref(),
        Some(r#"{"files":{"app.rb":"old"}}"#)
    );
}

#[test]
fn a_complete_run_records_the_hash_of_the_source_files() {
    let url = server(
        "200 OK",
        r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"{\"domains\":[]}"}}]}"#,
    );
    let dir = repo(&url);

    let output = generate(dir.path());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "{stdout}");
    let hashes = saved(dir.path()).expect("source-hashes.json written");
    assert!(hashes.contains("app.rb"), "{hashes}");
}
