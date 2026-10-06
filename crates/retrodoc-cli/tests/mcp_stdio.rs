//! `retrodoc mcp` as an agent runs it: the real binary on stdin/stdout.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use retrodoc_core::config::Config;
use retrodoc_core::model::Feature;
use retrodoc_pipeline::{DomainCluster, DomainMap};

fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    Config::write_default(dir.path(), false).unwrap();
    let feature = Feature {
        slug: "pay-invoice".into(),
        domain_slug: "billing".into(),
        sub_domain_slug: None,
        name: "Pay an invoice".into(),
        description: "Settle what is due".into(),
        source_paths: vec!["app/invoice.rb".into()],
        confidence: None,
    };
    retrodoc_pipeline::save_features(dir.path(), &[feature]).unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            description: "Invoices and payments".into(),
            paths: vec![],
            sub_domains: vec![],
        }],
    }
    .save(dir.path())
    .unwrap();
    dir
}

#[test]
fn the_server_speaks_only_the_protocol_on_stdout() {
    let repo = generated_repo();
    let mut child = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["mcp", "--path"])
        .arg(repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let (lines, received) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    let next_line = || {
        received
            .recv_timeout(Duration::from_secs(20))
            .expect("the server answered within 20 s")
    };

    let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
    let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_domain","arguments":{"id":"billing"}}}"#;
    writeln!(stdin, "{initialize}").unwrap();
    let first = next_line();
    assert!(
        serde_json::from_str::<serde_json::Value>(&first).is_ok(),
        "stdout must carry JSON only, got: {first}"
    );
    assert!(first.contains(r#""name":"retrodoc""#), "{first}");
    writeln!(stdin, "{initialized}").unwrap();
    writeln!(stdin, "{call}").unwrap();
    let answer = next_line();
    assert!(
        serde_json::from_str::<serde_json::Value>(&answer).is_ok(),
        "stdout must carry JSON only, got: {answer}"
    );
    assert!(answer.contains("billing/pay-invoice"), "{answer}");

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn a_repo_that_was_never_generated_is_refused_with_the_way_out() {
    let dir = tempfile::tempdir().unwrap();
    Config::write_default(dir.path(), false).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["mcp", "--path"])
        .arg(dir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("run `retrodoc generate` first"));
    assert_eq!(output.stdout, Vec::<u8>::new());
}
