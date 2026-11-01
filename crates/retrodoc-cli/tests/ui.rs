//! `retrodoc ui` as a user runs it: the real binary, asked over a socket.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
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
        slug: "pay-invoice".into(),
        domain_slug: "billing".into(),
        name: "Pay an invoice".into(),
        description: "Settle what is due".into(),
        source_paths: vec![],
        entry_points: Vec::new(),
        confidence: None,
    };
    retrodoc_pipeline::save_features(dir.path(), &[feature]).unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            description: "Invoices and payments".into(),
            ..DomainCluster::default()
        }],
    }
    .save(dir.path())
    .unwrap();
    dir
}

/// Kills the server when the test ends, passed or failed.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts `retrodoc ui` on `bind` and waits for the URL it prints.
fn start(repo: &tempfile::TempDir, bind: &str) -> (Server, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["ui", "--bind", bind, "--path"])
        .arg(repo.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let server = Server(child);
    let (lines, received) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    loop {
        let line = received
            .recv_timeout(Duration::from_secs(30))
            .expect("no URL printed");
        if let Some(at) = line.find("http://") {
            let url = line[at..].split_whitespace().next().unwrap().to_string();
            return (server, url);
        }
    }
}

/// The status line and body of `GET /` at `url` (`http://host:port/`).
fn get_home(url: &str) -> (String, String) {
    let host = url
        .strip_prefix("http://")
        .unwrap()
        .trim_end_matches('/')
        .to_string();
    let mut stream = TcpStream::connect(&host).unwrap();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    (head.lines().next().unwrap().to_string(), body.to_string())
}

// spec: WEB-2, WEB-3
#[test]
fn the_server_listens_on_the_address_given_and_prints_its_url() {
    let repo = generated_repo();
    let (_server, url) = start(&repo, "127.0.0.1:0");
    assert!(url.starts_with("http://127.0.0.1:"), "{url}");
    assert!(url.ends_with('/'), "{url}");
    assert!(!url.ends_with(":0/"), "the port picked is printed: {url}");
    let (status, body) = get_home(&url);
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(body.contains("Invoices and payments"), "{body}");
}

// spec: WEB-9
#[test]
fn a_repository_never_initialized_asks_for_init() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["ui", "--bind", "127.0.0.1:0", "--path"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("retrodoc init"), "{text}");
}
