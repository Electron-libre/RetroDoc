//! `[llm.passes.<name>]`: a pass calls its own server and model, the others keep `[llm]`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A chat-completions server answering nothing useful, counting its requests.
fn server(model: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/chat/completions", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buf = vec![0u8; 65536];
            let _ = stream.read(&mut buf);
            counter.fetch_add(1, Ordering::SeqCst);
            let payload = format!(
                r#"{{"model":"{model}","choices":[{{"message":{{"role":"assistant","content":"not json"}}}}]}}"#
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (url, hits)
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

#[test]
fn a_pass_calls_its_own_server_and_the_recap_names_its_model() {
    let (default_url, default_hits) = server("default-model");
    let (roles_url, roles_hits) = server("roles-model");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("app.rb"), "class Order; end\n").unwrap();
    std::fs::write(
        dir.path().join("retrodoc.toml"),
        format!(
            "[llm]\napi_key_env = \"RETRODOC_TEST_PASS_MODELS_KEY\"\nmodel = \"default-model\"\n\
             base_url = \"{default_url}\"\n\n\
             [llm.passes.roles]\nmodel = \"roles-model\"\nbase_url = \"{roles_url}\"\n"
        ),
    )
    .unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);

    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["roles", "--path"])
        .arg(dir.path())
        .env("RETRODOC_TEST_PASS_MODELS_KEY", "unused")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(roles_hits.load(Ordering::SeqCst) >= 1, "{stdout}");
    assert_eq!(default_hits.load(Ordering::SeqCst), 0, "{stdout}");
}

#[test]
fn an_unknown_pass_stops_the_command_before_any_call() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("retrodoc.toml"),
        "[llm.passes.domain]\nmodel = \"x\"\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["roles", "--path"])
        .arg(dir.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("domain") && stderr.contains("use-cases"),
        "{stderr}"
    );
}
