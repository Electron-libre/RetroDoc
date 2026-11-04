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
        back_office: false,
        technical: false,
        slug: "pay-invoice".into(),
        domain_slug: "billing".into(),
        name: "Pay an invoice".into(),
        description: "Settle what is due".into(),
        source_paths: vec!["app/invoice.rb".into()],
        entry_points: Vec::new(),
        confidence: None,
    };
    retrodoc_pipeline::save_features(dir.path(), &[feature]).unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            description: "Invoices and payments".into(),
            paths: vec![],
            ..DomainCluster::default()
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

/// `retrodoc mcp` on `repo`, initialized, answering one request at a time.
struct Session {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    lines: mpsc::Receiver<String>,
    next_id: u32,
}

impl Session {
    fn start(repo: &tempfile::TempDir) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
            .args(["mcp", "--path"])
            .arg(repo.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let mut session = Self {
            child,
            stdin,
            lines,
            next_id: 1,
        };
        session.request(
            "initialize",
            r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}"#,
        );
        writeln!(
            session.stdin,
            r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
        )
        .unwrap();
        session
    }

    fn request(&mut self, method: &str, params: &str) -> String {
        let id = self.next_id;
        self.next_id += 1;
        writeln!(
            self.stdin,
            r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#
        )
        .unwrap();
        self.lines
            .recv_timeout(Duration::from_secs(20))
            .expect("the server answered within 20 s")
    }

    /// The text the tool `name` answers with `arguments`.
    fn call(&mut self, name: &str, arguments: &str) -> String {
        let answer = self.request(
            "tools/call",
            &format!(r#"{{"name":"{name}","arguments":{arguments}}}"#),
        );
        let answer: serde_json::Value = serde_json::from_str(&answer).unwrap();
        answer["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("no text in {answer}"))
            .to_string()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// spec: MCP-14
#[test]
fn the_answers_show_the_edited_sections() {
    let repo = generated_repo();
    std::fs::write(
        repo.path().join(".retrodoc/edits.json"),
        r#"{"version":1,"edits":[
            {"kind":"domain","id":"billing","name":"Invoicing","author":null,
             "edited_at":"2026-11-05T10:00:00Z","fingerprint":"f"},
            {"kind":"feature","id":"pay-invoice","description":"Settle the invoice by card","author":null,
             "edited_at":"2026-11-05T10:00:00Z","fingerprint":"f"}]}"#,
    )
    .unwrap();
    let mut session = Session::start(&repo);

    let domains = session.call("list_domains", "{}");
    assert!(domains.contains("Invoicing"), "{domains}");
    let feature = session.call("get_feature", r#"{"id":"billing/pay-invoice"}"#);
    assert!(feature.contains("Settle the invoice by card"), "{feature}");
    assert!(!feature.contains("Settle what is due"), "{feature}");
    assert!(
        feature.contains("Pay an invoice"),
        "the name is not edited: {feature}"
    );
}
