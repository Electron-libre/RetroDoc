//! `retrodoc ui --ask` as a user runs it: the real binary asked over a socket, the model a fake
//! LLM server (`specs/web-ask.md`).

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use retrodoc_core::model::Feature;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use support::{llm_section, repo_with_config, FakeLlm};

/// A repository whose last `generate` documented one feature grounded on `app/order.rb`.
fn generated_repo(config: &str) -> tempfile::TempDir {
    let dir = repo_with_config(&[("app/order.rb", "class Order; end\n")], config);
    let feature = Feature {
        back_office: false,
        technical: false,
        slug: "list-orders".into(),
        domain_slug: "orders".into(),
        name: "List orders".into(),
        description: "The customer sees their orders".into(),
        source_paths: vec!["app/order.rb".into()],
        entry_points: Vec::new(),
        confidence: None,
    };
    retrodoc_pipeline::save_features(dir.path(), &[feature]).unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "orders".into(),
            name: "Orders".into(),
            description: "Orders of the shop".into(),
            ..DomainCluster::default()
        }],
    }
    .save(dir.path())
    .unwrap();
    dir
}

fn ui(dir: &Path, ask: bool) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_retrodoc"));
    command
        .args(["ui", "--bind", "127.0.0.1:0", "--path"])
        .arg(dir);
    if ask {
        command.arg("--ask");
    }
    command
        .env("RETRODOC_TEST_FAKE_LLM_KEY", "unused")
        .env_remove("RUST_LOG");
    command
}

/// A server started, the lines it printed after its URL kept for the end; killed when dropped.
struct Server {
    child: Child,
    url: String,
    lines: mpsc::Receiver<String>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    fn start(mut command: Command) -> Self {
        let mut child = command.stdout(Stdio::piped()).spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let url = loop {
            let line = lines
                .recv_timeout(Duration::from_secs(30))
                .expect("no URL printed");
            if let Some(at) = line.find("http://") {
                break line[at..].split_whitespace().next().unwrap().to_string();
            }
        };
        Self { child, url, lines }
    }

    /// The status line and body of `request` (`GET /` or `POST /ask`) with `body`.
    fn send(&self, request: &str, body: &str) -> (String, String) {
        let host = self
            .url
            .strip_prefix("http://")
            .unwrap()
            .trim_end_matches('/');
        let mut stream = TcpStream::connect(host).unwrap();
        write!(
            stream,
            "{request} HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nHX-Request: true\r\n\
             Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        (head.lines().next().unwrap().to_string(), body.to_string())
    }

    /// Sends the server the signal of Ctrl-C.
    fn signal(&self) {
        let killed = Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .status()
            .unwrap();
        assert!(killed.success());
    }

    /// Stops the server as Ctrl-C does, and gives what it printed after its URL.
    fn interrupt(mut self) -> String {
        self.signal();
        let status = self.child.wait().unwrap();
        assert!(status.success(), "{status}");
        self.lines.iter().collect::<Vec<_>>().join("\n")
    }
}

fn failure(output: &Output) -> String {
    assert!(!output.status.success());
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// `[llm]` reading its key from a variable no test sets.
const NO_KEY: &str = "[llm]\napi_key_env = \"RETRODOC_TEST_UNSET_KEY\"\nmodel = \"fake\"\n";

// spec: WAK-1
#[test]
fn without_ask_the_server_starts_with_no_api_key() {
    let dir = generated_repo(NO_KEY);

    let server = Server::start(ui(dir.path(), false));

    assert_eq!(server.send("GET /", "").0, "HTTP/1.1 200 OK");
}

// spec: WAK-3
#[test]
fn with_ask_and_no_generated_docs_the_server_does_not_start() {
    let llm = FakeLlm::start(|_| "unused".to_string());
    let dir = repo_with_config(
        &[("app/order.rb", "class Order; end\n")],
        &llm_section(&llm),
    );

    let stderr = failure(&ui(dir.path(), true).output().unwrap());

    assert!(stderr.contains("run `retrodoc generate` first"), "{stderr}");
}

// spec: WAK-4
#[test]
fn with_ask_and_no_api_key_the_server_does_not_start() {
    let dir = generated_repo(NO_KEY);

    let stderr = failure(&ui(dir.path(), true).output().unwrap());

    assert!(stderr.contains("`ask` pass"), "{stderr}");
    assert!(stderr.contains("RETRODOC_TEST_UNSET_KEY"), "{stderr}");
}

// spec: WAK-5, WAK-23, WAK-24
#[test]
fn a_question_uses_the_ask_pass_and_its_recap_is_saved_when_the_server_stops() {
    let default = FakeLlm::start(|_| "unused".to_string());
    let ask = FakeLlm::start(|_| "They are listed in `orders/list-orders`.".to_string());
    let dir = generated_repo(&format!(
        "{}[llm.passes.ask]\nmodel = \"ask-model\"\nbase_url = \"{}\"\n",
        llm_section(&default),
        ask.url
    ));
    support::git(dir.path(), &["add", "-A"]);
    support::git(dir.path(), &["commit", "-qm", "docs"]);
    let server = Server::start(ui(dir.path(), true));

    let (status, exchange) = server.send("POST /ask", "question=How+are+orders+listed%3F");
    let out = server.interrupt();

    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(exchange.contains("They are listed in"), "{exchange}");
    assert_eq!(ask.calls().len(), 1);
    assert!(default.calls().is_empty());
    assert!(
        out.lines()
            .any(|l| l.contains("ask") && l.contains("1 call")),
        "{out}"
    );
    let usage = std::fs::read_to_string(dir.path().join(".retrodoc/cache/usage.json")).unwrap();
    assert!(usage.contains("\"ui\""), "{usage}");
    let changed = Command::new("git")
        .args(["status", "--porcelain", "--", ".", ":(exclude).retrodoc"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&changed.stdout), "");
}

// spec: WAK-28
#[test]
fn a_second_interrupt_drops_the_question_in_flight() {
    let llm = FakeLlm::start(|_| {
        std::thread::sleep(Duration::from_secs(60));
        "too late".to_string()
    });
    let dir = generated_repo(&llm_section(&llm));
    let server = Server::start(ui(dir.path(), true));
    let host = server.url.clone();
    std::thread::spawn(move || {
        let host = host
            .strip_prefix("http://")
            .unwrap()
            .trim_end_matches('/')
            .to_string();
        let mut stream = TcpStream::connect(&host).unwrap();
        let body = "question=Hi";
        let _ = write!(
            stream,
            "POST /ask HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\n\
             Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.read_to_end(&mut Vec::new());
    });
    let start = std::time::Instant::now();
    while llm.calls().is_empty() {
        assert!(start.elapsed().as_secs() < 30, "no call");
        std::thread::sleep(Duration::from_millis(20));
    }
    server.signal();
    std::thread::sleep(Duration::from_millis(300));

    let out = server.interrupt();

    assert!(
        start.elapsed().as_secs() < 30,
        "the question was waited for"
    );
    assert!(out.contains("Ctrl+C again"), "{out}");
}
