//! A fake chat-completions server scripted per pass, and the repositories `generate` runs on.
//!
//! The server tells the passes apart by the schema name of their structured output (the system
//! prompt for the three that share `Response`), answers from a script and keeps every request.

#![allow(dead_code)] // each test binary uses a part of it

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

/// One request the server received.
#[derive(Debug, Clone)]
pub struct Call {
    /// The pass, named after its schema (see [`pass_of`]).
    pub pass: String,
    pub system: String,
    pub user: String,
}

impl Call {
    /// The files of a reading batch, framed `--- path ---`, with their text.
    pub fn files(&self) -> Vec<(String, String)> {
        let mut files: Vec<(String, String)> = Vec::new();
        for line in self.user.lines() {
            if let Some(path) = line
                .strip_prefix("--- ")
                .and_then(|l| l.strip_suffix(" ---"))
            {
                files.push((path.to_string(), String::new()));
            } else if let Some((_, text)) = files.last_mut() {
                text.push_str(line);
                text.push('\n');
            }
        }
        files
    }
}

/// The fake server: its URL and the requests it received.
pub struct FakeLlm {
    pub url: String,
    calls: Arc<Mutex<Vec<Call>>>,
}

impl FakeLlm {
    /// Serves `answer(call)` as the content of each completion.
    pub fn start(answer: impl Fn(&Call) -> String + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/chat/completions", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&calls);
        let answer = Arc::new(answer);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let (kept, answer) = (Arc::clone(&kept), Arc::clone(&answer));
                std::thread::spawn(move || serve(stream, &kept, answer.as_ref()));
            }
        });
        Self { url, calls }
    }

    /// The requests received so far, in order.
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    /// The requests of `pass`.
    pub fn calls_of(&self, pass: &str) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|c| c.pass == pass)
            .collect()
    }
}

/// Answers the requests of one connection (the client keeps it alive) until it closes.
fn serve(
    stream: TcpStream,
    kept: &Mutex<Vec<Call>>,
    answer: &(dyn Fn(&Call) -> String + Send + Sync),
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut stream = stream;
    while let Some(body) = read_request(&mut reader) {
        let call = call_of(&body);
        kept.lock().unwrap().push(call.clone());
        // A pass with no script answers a 500 naming it, rather than a dropped connection.
        let Ok(content) = std::panic::catch_unwind(AssertUnwindSafe(|| answer(&call))) else {
            let message = format!("no script for the pass {}", call.pass);
            let response = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\n\r\n{message}",
                message.len()
            );
            let _ = stream.write_all(response.as_bytes());
            return;
        };
        let payload = json!({
            "model": "fake",
            "choices": [{"message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            payload.len()
        );
        if stream.write_all(response.as_bytes()).is_err() {
            return;
        }
    }
}

/// The body of the next request, `None` once the connection is closed.
fn read_request(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().ok()?;
            }
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn call_of(body: &Value) -> Call {
    let message = |i: usize| {
        body["messages"][i]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    let (system, user) = (message(0), message(1));
    let schema = body["response_format"]["json_schema"]["name"]
        .as_str()
        .unwrap_or_default();
    Call {
        pass: pass_of(schema, &system).to_string(),
        system,
        user,
    }
}

/// The pass a request comes from, by its schema name.
fn pass_of<'s>(schema: &'s str, system: &str) -> &'s str {
    match schema {
        "Response" if system.contains("tell what the product does") => "sources",
        "Response" if system.contains("where the business lives") => "business-files",
        "Response" => "contexts",
        "RawBrief" => "brief",
        "RolesResponse" => "roles",
        "UnmatchedRolesResponse" => "roles-unmatched",
        "PatternResponse" => "roles-boundary",
        "BusinessReadResponse" => "business-read",
        "GlossaryResponse" => "glossary",
        "EntryPointsResponse" => "entry-points",
        "RawActors" => "actors",
        "RawSurfaceDomains" => "domains",
        "RawSurfaceFeatures" => "features",
        "RawPlacements" => "features-repair",
        "RawUseCases" => "use-cases",
        "RawFeatureKind" => "rules-kind",
        "RawRules" => "rules",
        "RawVerdict" => "rules-judge",
        "RawBatchVerdicts" | "RawVerdicts" => "confidence",
        "RawAnswer" => "reconcile",
        other => other,
    }
}

/// A minimal application: what the fake LLM answers about it, pass by pass.
#[derive(Debug, Clone)]
pub struct App {
    pub stack: &'static str,
    /// The extensions the roles call names as source.
    pub source_extensions: Vec<&'static str>,
    /// `(extension, regex)` the roles call gives to cut long files.
    pub chunk_boundaries: Vec<(&'static str, &'static str)>,
    /// The folder of the business code, with its trailing `/`.
    pub business: &'static str,
    /// The file of the entry point and the file of the model.
    pub entry: &'static str,
    pub model: &'static str,
}

impl App {
    /// The answer of the fake LLM to `call`.
    #[allow(clippy::too_many_lines)] // one arm per pass
    pub fn answer(&self, call: &Call) -> String {
        let answer = match call.pass.as_str() {
            "sources" => json!({"rules": []}),
            "brief" => json!({"purpose": "Sells products to customers."}),
            "roles" => json!({
                "stack": self.stack,
                "rules": [{"pattern": format!("{}**", self.business), "role": "logic"}],
                "source_extensions": self.source_extensions,
                "chunk_boundaries": self.chunk_boundaries.iter()
                    .map(|(ext, pattern)| json!({"extensions": [ext], "pattern": pattern}))
                    .collect::<Vec<_>>(),
            }),
            // Every file left is given a role, as a model would.
            "roles-unmatched" => json!({"rules": [{"pattern": "**", "role": "other"}]}),
            "roles-boundary" => json!({"pattern": "^\\s*def\\s"}),
            "business-files" => {
                json!({"business": [{"path": self.business, "reason": "the shop"}]})
            }
            "business-read" | "glossary" | "entry-points" => self.read(call),
            "actors" => {
                json!({"actors": [{"name": "Customer", "kind": "human", "description": "Buys."}]})
            }
            "domains" => json!({"domains": [{
                "slug": "orders", "name": "Orders", "description": "Orders of the shop.",
                "resources": ["order"], "entities": ["Order"],
            }]}),
            "features" => json!({"features": [{
                "slug": "list-orders", "name": "List orders", "description": "Lists the orders.",
                "back_office": false, "entry_points": ["E1"],
            }]}),
            "features-repair" => json!({"placements": []}),
            "use-cases" => json!({"use_cases": [{
                "slug": "list-orders", "name": "List orders", "description": "Lists the orders.",
                "steps": [{"actor": "Customer", "text": "Asks for the orders.", "code": ["F1:1-1"]}],
                "entry_points": ["E1"], "primary_actor": "Customer", "narrative": "The customer lists the orders.",
            }]}),
            "rules-kind" => json!({"about_tests": false}),
            "rules" => json!({"rules": [{
                "statement": "An order lists its products.", "kind": "validation", "use_cases": [],
                "code": ["F1:1-1"], "tests": [], "docs": [],
            }]}),
            "rules-judge" => json!({"rules": [{"n": 1, "class": "business"}]}),
            "confidence" => json!({
                "steps": [{"order": 1, "verdict": "supported", "rationale": "The code does it."}],
                "use_cases": [{"slug": "list-orders", "steps": [{"order": 1, "verdict": "supported", "rationale": "Yes."}]}],
            }),
            "reconcile" => json!({"items": []}),
            other => panic!("no script for the pass {other}"),
        };
        answer.to_string()
    }

    /// One entry point in the entry file, one entity in the model file, of the files read.
    fn read(&self, call: &Call) -> Value {
        let (mut entry_points, mut entities) = (Vec::new(), Vec::new());
        for (file, _) in call.files() {
            if file == self.entry {
                entry_points.push(json!({
                    "file": file, "kind": "http_route", "name": "GET /orders", "verb": "list",
                    "resource": "order", "description": "Lists the orders.", "outputs": [],
                }));
            }
            if file == self.model {
                entities.push(json!({
                    "file": file, "name": "Order", "description": "An order of a customer.",
                    "attributes": ["total"], "associations": [],
                }));
            }
        }
        json!({"entry_points": entry_points, "entities": entities})
    }
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

/// A repository of `files` with one commit, its `retrodoc.toml` pointing at `llm`.
pub fn repo(files: &[(&str, &str)], llm: &FakeLlm) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (file, text) in files {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    std::fs::write(
        dir.path().join("retrodoc.toml"),
        format!(
            "[llm]\napi_key_env = \"RETRODOC_TEST_FAKE_LLM_KEY\"\nmodel = \"fake\"\nbase_url = \"{}\"\n",
            llm.url
        ),
    )
    .unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["add", "."]);
    git(
        dir.path(),
        &["commit", "-q", "-m", "Let customers list their orders"],
    );
    dir
}

/// Runs `generate` on `dir`.
pub fn generate(dir: &Path) -> Output {
    retrodoc(dir, "generate")
}

/// Runs the subcommand `command` on `dir`.
pub fn retrodoc(dir: &Path, command: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args([command, "--path"])
        .arg(dir)
        .current_dir(dir)
        .env("RETRODOC_TEST_FAKE_LLM_KEY", "unused")
        .env_remove("RUST_LOG")
        .output()
        .unwrap()
}

/// The stdout of the subcommand `command` on `dir`, which must succeed.
pub fn stdout_of(dir: &Path, command: &str) -> String {
    let output = retrodoc(dir, command);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

/// The ` WARN ` lines of a run.
pub fn warnings(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains(" WARN "))
        .map(str::to_string)
        .collect()
}
