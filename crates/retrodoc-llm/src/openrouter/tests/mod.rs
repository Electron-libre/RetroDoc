use std::sync::atomic::AtomicBool;

use super::{OpenRouterProvider, DEFAULT_REQUEST_TIMEOUT};
use crate::CompletionRequest;

mod completion;
mod config;
mod fallbacks;
mod retry;
mod timeouts;

pub(super) fn request() -> CompletionRequest {
    CompletionRequest {
        messages: Vec::new(),
        model: None,
        json_schema: None,
    }
}

/// A provider with the test defaults, sending to `endpoint`; tests override a
/// field with struct-update syntax.
pub(super) fn provider(endpoint: &str) -> OpenRouterProvider {
    OpenRouterProvider {
        client: reqwest::Client::new(),
        api_key: "test-key".to_string(),
        default_model: "m".to_string(),
        endpoint: String::new(),
        reasoning_effort: None,
        temperature: None,
        structured_output: true,
        require_parameters: false,
        schema_refused: AtomicBool::new(false),
        temperature_refused: AtomicBool::new(false),
        timeout: DEFAULT_REQUEST_TIMEOUT,
    }
    .with_endpoint(endpoint)
}

/// Serves the given raw HTTP responses in order, one per connection, and
/// returns the requests received.
pub(super) fn serve(responses: Vec<String>) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::Write;
        for payload in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let _ = tx.send(read_request(&mut stream));
            let _ = stream.write_all(payload.as_bytes());
        }
    });
    (format!("http://{addr}"), rx)
}

/// Reads one request whole, its headers then the `Content-Length` bytes of
/// its body, which may come in several segments.
fn read_request(stream: &mut std::net::TcpStream) -> String {
    use std::io::Read;
    let mut received = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let text = String::from_utf8_lossy(&received);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            if received.len() >= end + 4 + length {
                return text.into_owned();
            }
        }
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return String::from_utf8_lossy(&received).into_owned(),
            Ok(n) => received.extend_from_slice(&buf[..n]),
        }
    }
}

pub(super) fn http(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

pub(super) const OK_BODY: &str =
    r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"{}"}}]}"#;
