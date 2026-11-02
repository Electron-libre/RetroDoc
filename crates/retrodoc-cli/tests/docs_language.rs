//! `[output] language`: the writing passes are asked in it, the reading passes are not
//! (`specs/docs-language.md`).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};

/// A chat-completions server answering nothing useful, keeping the system prompt of each request.
fn server() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/chat/completions", listener.local_addr().unwrap());
    let systems = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&systems);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; length];
            let _ = reader.read_exact(&mut body);
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            if let Some(system) = request["messages"][0]["content"].as_str() {
                seen.lock().unwrap().push(system.to_string());
            }
            let payload = r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"not json"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (url, systems)
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=A", "-c", "user.email=a@example.com"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

// spec: DLG-2, DLG-3
#[test]
fn the_brief_is_asked_in_the_language_and_the_sources_are_not() {
    let (url, systems) = server();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("README.md"),
        "# Shop\n\nBuyers pay for their orders.\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("app.rb"), "class Order; end\n").unwrap();
    std::fs::write(
        dir.path().join("retrodoc.toml"),
        format!(
            "[llm]\napi_key_env = \"RETRODOC_TEST_DOCS_LANGUAGE_KEY\"\nmodel = \"m\"\nbase_url = \"{url}\"\n\n\
             [output]\nlanguage = \"fr\"\n"
        ),
    )
    .unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);

    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["brief", "--path"])
        .arg(dir.path())
        .env("RETRODOC_TEST_DOCS_LANGUAGE_KEY", "unused")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let systems = systems.lock().unwrap();
    let instruction = "in French. Keep exactly as given";
    let brief: Vec<&String> = systems
        .iter()
        .filter(|s| s.starts_with("You are writing the product brief"))
        .collect();
    let sources: Vec<&String> = systems
        .iter()
        .filter(|s| s.starts_with("You are locating"))
        .collect();
    assert!(!brief.is_empty(), "{stdout}\n{systems:?}");
    assert!(!sources.is_empty(), "{stdout}\n{systems:?}");
    assert!(brief.iter().all(|s| s.contains(instruction)), "{brief:?}");
    assert!(
        sources.iter().all(|s| !s.contains(instruction)),
        "{sources:?}"
    );
}
