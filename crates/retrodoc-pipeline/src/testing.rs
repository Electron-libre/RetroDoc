//! A fake [`LlmProvider`] for the tests of the passes, so that none of them
//! needs a network, nor its own copy of the same few lines.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use retrodoc_llm::{
    ChatMessage, CompletionRequest, CompletionResponse, LlmError, LlmProvider, ToolCall,
};
use tracing_subscriber::fmt::MakeWriter;

type Reply = Box<dyn Fn(usize, &CompletionRequest) -> Result<FakeTurn, LlmError> + Send + Sync>;

/// One answer of a [`FakeLlm::scripted`] provider.
#[derive(Debug, Clone)]
pub enum FakeTurn {
    /// A text answer.
    Text(String),
    /// Calls to tools, with an empty text.
    Calls(Vec<ToolCall>),
}

/// Answers with what its reply function says, counts its calls and keeps the
/// prompts it receives.
pub struct FakeLlm {
    reply: Reply,
    calls: AtomicUsize,
    prompts: Mutex<Vec<(String, String)>>,
    messages: Mutex<Vec<Vec<ChatMessage>>>,
    schemas: Mutex<Vec<Option<serde_json::Value>>>,
    model: String,
    /// How long a call takes (none: it answers at once), to make calls overlap.
    delay: Option<std::time::Duration>,
    in_flight: AtomicUsize,
    most_in_flight: AtomicUsize,
    /// Calls from this rank on never answer.
    hang_from: Option<usize>,
    /// A call is waiting for an answer that never comes.
    hanging: std::sync::atomic::AtomicBool,
    /// A user prompt longer than this gets a half answer flagged as cut by the completion limit.
    truncate_over: Option<usize>,
}

impl FakeLlm {
    /// Always the same answer.
    pub fn answering(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::replying(move |_, _| Ok(text.clone()))
    }

    /// Each answer in turn; the last one repeats.
    pub fn sequence(texts: &[&str]) -> Self {
        let texts: Vec<String> = texts.iter().map(ToString::to_string).collect();
        Self::replying(move |n, _| Ok(texts[n.min(texts.len() - 1)].clone()))
    }

    /// Each turn in order, tool calls or text, as the rounds of a tool loop; the last one repeats.
    pub fn scripted(turns: Vec<FakeTurn>) -> Self {
        Self::with_reply(Box::new(move |n, _| {
            Ok(turns[n.min(turns.len() - 1)].clone())
        }))
    }

    /// Answers from the rank of the call (from 0) and the request.
    pub fn replying(
        reply: impl Fn(usize, &CompletionRequest) -> Result<String, LlmError> + Send + Sync + 'static,
    ) -> Self {
        Self::with_reply(Box::new(move |n, request| {
            reply(n, request).map(FakeTurn::Text)
        }))
    }

    fn with_reply(reply: Reply) -> Self {
        Self {
            reply,
            calls: AtomicUsize::new(0),
            prompts: Mutex::new(Vec::new()),
            messages: Mutex::new(Vec::new()),
            schemas: Mutex::new(Vec::new()),
            model: "test-model".to_string(),
            delay: None,
            in_flight: AtomicUsize::new(0),
            most_in_flight: AtomicUsize::new(0),
            hang_from: None,
            hanging: std::sync::atomic::AtomicBool::new(false),
            truncate_over: None,
        }
    }

    /// Answers `reply` to the first `succeed` calls, then never answers: with [`killed`], a run
    /// stopped while it waits for the LLM.
    pub fn hanging_after(
        succeed: usize,
        reply: impl Fn(usize) -> String + Send + Sync + 'static,
    ) -> Self {
        let mut llm = Self::replying(move |n, _| Ok(reply(n)));
        llm.hang_from = Some(succeed);
        llm
    }

    /// Makes each call wait, so that calls asked together are in flight together.
    pub fn with_delay(mut self, delay: std::time::Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// Cuts the answer, as a server at its completion limit does, of each call whose user prompt
    /// is longer than `chars`.
    pub fn truncating_prompts_over(mut self, chars: usize) -> Self {
        self.truncate_over = Some(chars);
        self
    }

    /// The most calls that were in flight at once.
    pub fn most_in_flight(&self) -> usize {
        self.most_in_flight.load(Ordering::SeqCst)
    }

    /// The model it says it uses.
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    /// Answers `reply` to the first `succeed` calls, then fails like a
    /// dropped connection.
    pub fn failing_after(
        succeed: usize,
        reply: impl Fn(usize) -> String + Send + Sync + 'static,
    ) -> Self {
        Self::replying(move |n, _| {
            if n < succeed {
                Ok(reply(n))
            } else {
                Err(LlmError::Transport("down".to_string()))
            }
        })
    }

    /// Calls received so far.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// The user prompt of each call received.
    pub fn prompts(&self) -> Vec<String> {
        self.prompts
            .lock()
            .unwrap()
            .iter()
            .map(|(_, user)| user.clone())
            .collect()
    }

    /// The system prompt of each call received.
    pub fn system_prompts(&self) -> Vec<String> {
        self.prompts
            .lock()
            .unwrap()
            .iter()
            .map(|(system, _)| system.clone())
            .collect()
    }

    /// The messages of each call received, tool calls and results included.
    pub fn messages(&self) -> Vec<Vec<ChatMessage>> {
        self.messages.lock().unwrap().clone()
    }

    /// The JSON schema each call received asked for, if any.
    pub fn schemas(&self) -> Vec<Option<serde_json::Value>> {
        self.schemas.lock().unwrap().clone()
    }

    /// The (system, user) prompts of each call received.
    pub fn prompt_pairs(&self) -> Vec<(String, String)> {
        self.prompts.lock().unwrap().clone()
    }
}

#[async_trait]
impl LlmProvider for FakeLlm {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.hang_from.is_some_and(|from| n >= from) {
            self.hanging.store(true, Ordering::SeqCst);
            std::future::pending::<()>().await;
        }
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.most_in_flight.fetch_max(now, Ordering::SeqCst);
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        let prompt = |i: usize| {
            request
                .messages
                .get(i)
                .map(|m| m.content.clone())
                .unwrap_or_default()
        };
        self.prompts.lock().unwrap().push((prompt(0), prompt(1)));
        self.messages.lock().unwrap().push(request.messages.clone());
        self.schemas
            .lock()
            .unwrap()
            .push(request.json_schema.as_ref().map(|s| s.schema.clone()));
        let (mut content, tool_calls) = match (self.reply)(n, &request)? {
            FakeTurn::Text(text) => (text, Vec::new()),
            FakeTurn::Calls(calls) => (String::new(), calls),
        };
        let truncated = self
            .truncate_over
            .is_some_and(|over| prompt(1).len() > over);
        if truncated {
            content.truncate(content.len() / 2);
        }
        Ok(CompletionResponse {
            content,
            model: self.model.clone(),
            truncated,
            tool_calls,
            ..Default::default()
        })
    }

    fn model(&self) -> &str {
        &self.model
    }
}

/// Runs `pass` until its LLM (see [`FakeLlm::hanging_after`]) is waiting for an answer that never
/// comes, then drops it as a kill of the process would: nothing runs after the last call answered,
/// no error path either. What the disk holds afterwards is what the next run starts from. It waits
/// for the call itself, not for a delay, so a slow machine cannot cut the pass short.
pub(crate) async fn killed<T>(pass: impl std::future::Future<Output = T>, llm: &FakeLlm) {
    let waiting = async {
        while !llm.hanging.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    };
    let guard = tokio::time::sleep(std::time::Duration::from_secs(30));
    tokio::select! {
        _ = pass => panic!("the pass ended instead of waiting for the LLM"),
        () = waiting => {}
        () = guard => panic!("the LLM was never asked a call it leaves unanswered"),
    }
}

/// A brief with only a purpose, to check that a pass reads it.
pub fn brief(purpose: &str) -> crate::brief::ProductBrief {
    crate::brief::ProductBrief {
        purpose: crate::brief::Claim {
            text: purpose.to_string(),
            sources: Vec::new(),
        },
        ..crate::brief::ProductBrief::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrodoc_llm::{ChatMessage, Role, ToolCall};

    fn request(system: &str, user: &str) -> CompletionRequest {
        let message = |role, content: &str| ChatMessage::new(role, content);
        CompletionRequest {
            messages: vec![message(Role::System, system), message(Role::User, user)],
            model: None,
            json_schema: None,
            tools: Vec::new(),
        }
    }

    #[tokio::test]
    async fn it_answers_counts_and_remembers_the_prompts() {
        let llm = FakeLlm::answering("yes");

        let first = llm.complete(request("s1", "u1")).await.unwrap();
        llm.complete(request("s2", "u2")).await.unwrap();

        assert_eq!(first.content, "yes");
        assert_eq!(llm.calls(), 2);
        assert_eq!(llm.prompts(), ["u1", "u2"]);
        assert_eq!(llm.prompt_pairs()[1], ("s2".to_string(), "u2".to_string()));
    }

    #[tokio::test]
    async fn a_sequence_repeats_its_last_answer() {
        let llm = FakeLlm::sequence(&["a", "b"]);
        let mut got = Vec::new();
        for _ in 0..3 {
            got.push(llm.complete(request("s", "u")).await.unwrap().content);
        }
        assert_eq!(got, ["a", "b", "b"]);
    }

    #[tokio::test]
    async fn it_fails_after_the_calls_it_was_given_to_answer() {
        let llm = FakeLlm::failing_after(1, |n| format!("ok {n}"));

        assert_eq!(
            llm.complete(request("s", "u")).await.unwrap().content,
            "ok 0"
        );
        assert!(matches!(
            llm.complete(request("s", "u")).await,
            Err(LlmError::Transport(_))
        ));
        // A failed call is still a call received.
        assert_eq!(llm.calls(), 2);
    }
    // spec: TCL-10 (internal: the fake provider is test support, which no binary runs)
    #[tokio::test]
    async fn it_answers_a_script_of_tool_calls_and_texts_and_keeps_the_messages() {
        let call = ToolCall {
            id: "call_1".to_string(),
            name: "list_domains".to_string(),
            arguments: "{}".to_string(),
        };
        let llm = FakeLlm::scripted(vec![
            FakeTurn::Calls(vec![call.clone()]),
            FakeTurn::Text("Billing.".to_string()),
        ]);

        let first = llm.complete(request("s", "Which domains?")).await.unwrap();
        assert_eq!(first.tool_calls, vec![call.clone()]);
        assert_eq!(first.content, "");
        let mut second = request("s", "Which domains?");
        second
            .messages
            .push(ChatMessage::assistant_calls("", first.tool_calls));
        second
            .messages
            .push(ChatMessage::tool_result("call_1", "- Billing"));
        let answer = llm.complete(second).await.unwrap();

        assert_eq!(answer.content, "Billing.");
        assert_eq!(answer.tool_calls, []);
        let received = llm.messages();
        assert_eq!(received.len(), 2);
        assert_eq!(received[1][2].tool_calls, vec![call]);
        assert_eq!(received[1][3].role, Role::Tool);
        assert_eq!(received[1][3].tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(received[1][3].content, "- Billing");
    }
}

/// What a pass logged while a test ran, as plain text (one line per event,
/// with its level), to check which events are warnings.
#[derive(Clone, Default)]
pub struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl LogBuffer {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl io::Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl MakeWriter<'_> for LogBuffer {
    type Writer = LogBuffer;

    fn make_writer(&self) -> Self::Writer {
        self.clone()
    }
}

thread_local! {
    static CAPTURE: std::cell::RefCell<Option<LogBuffer>> = const { std::cell::RefCell::new(None) };
}

/// Writer of the one global test subscriber: sends each event to the buffer
/// of the thread that logged it, and drops it when that thread captures none.
#[derive(Clone, Copy)]
struct ThreadWriter;

impl io::Write for ThreadWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        CAPTURE.with(|c| {
            if let Some(buffer) = c.borrow_mut().as_mut() {
                io::Write::write(buffer, bytes)?;
            }
            Ok(bytes.len())
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl MakeWriter<'_> for ThreadWriter {
    type Writer = ThreadWriter;

    fn make_writer(&self) -> Self::Writer {
        *self
    }
}

/// Stops the capture of the current thread when dropped.
pub struct CaptureGuard;

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        CAPTURE.with(|c| *c.borrow_mut() = None);
    }
}

/// Captures the logs of the current thread until the guard is dropped: use
/// it in a test on the default single-threaded runtime of `#[tokio::test]`.
///
/// One global subscriber serves every test: scoped subscribers made the
/// callsite interest cache of `tracing` race between parallel tests and lose
/// events.
pub fn capture_logs() -> (LogBuffer, CaptureGuard) {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(ThreadWriter)
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("no other test sets a global subscriber");
    });
    let buffer = LogBuffer::default();
    CAPTURE.with(|c| *c.borrow_mut() = Some(buffer.clone()));
    (buffer, CaptureGuard)
}

/// The walk of `root` as it is now.
pub(crate) fn walked(root: &std::path::Path) -> crate::walked::WalkedFiles {
    crate::walked::WalkedFiles::walk(root, &[]).unwrap()
}

/// A git repository holding `a.rs` and a gitignored `.env` whose content is `SECRET_TOKEN`: a file the walk skips.
pub(crate) fn repo_with_an_ignored_secret() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    // `.gitignore` only applies inside a git repository.
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::write(dir.path().join(".gitignore"), ".env\n").unwrap();
    std::fs::write(dir.path().join(".env"), "SECRET_TOKEN\n").unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    dir
}

/// The default context of the use cases and rules passes, reading the walk of `root` as it is now.
pub(crate) fn reading(root: &std::path::Path) -> crate::use_cases::UseCaseContext {
    crate::use_cases::UseCaseContext {
        walked: walked(root),
        ..crate::use_cases::UseCaseContext::default()
    }
}
