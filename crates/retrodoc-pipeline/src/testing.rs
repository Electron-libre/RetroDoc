//! A fake [`LlmProvider`] for the tests of the passes, so that none of them
//! needs a network, nor its own copy of the same few lines.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use retrodoc_llm::{CompletionRequest, CompletionResponse, LlmError, LlmProvider};
use tracing_subscriber::fmt::MakeWriter;

type Reply = Box<dyn Fn(usize, &CompletionRequest) -> Result<String, LlmError> + Send + Sync>;

/// Answers with what its reply function says, counts its calls and keeps the
/// prompts it receives.
pub(crate) struct FakeLlm {
    reply: Reply,
    calls: AtomicUsize,
    prompts: Mutex<Vec<(String, String)>>,
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

    /// Answers from the rank of the call (from 0) and the request.
    pub fn replying(
        reply: impl Fn(usize, &CompletionRequest) -> Result<String, LlmError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            reply: Box::new(reply),
            calls: AtomicUsize::new(0),
            prompts: Mutex::new(Vec::new()),
            schemas: Mutex::new(Vec::new()),
            model: "test-model".to_string(),
            delay: None,
            in_flight: AtomicUsize::new(0),
            most_in_flight: AtomicUsize::new(0),
            hang_from: None,
            hanging: std::sync::atomic::AtomicBool::new(false),
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
        self.schemas
            .lock()
            .unwrap()
            .push(request.json_schema.as_ref().map(|s| s.schema.clone()));
        let content = (self.reply)(n, &request)?;
        Ok(CompletionResponse {
            content,
            model: self.model.clone(),
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
pub(crate) fn brief(purpose: &str) -> crate::brief::ProductBrief {
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
    use retrodoc_llm::{ChatMessage, Role};

    fn request(system: &str, user: &str) -> CompletionRequest {
        let message = |role, content: &str| ChatMessage {
            role,
            content: content.to_string(),
        };
        CompletionRequest {
            messages: vec![message(Role::System, system), message(Role::User, user)],
            model: None,
            json_schema: None,
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
}

/// What a pass logged while a test ran, as plain text (one line per event,
/// with its level), to check which events are warnings.
#[derive(Clone, Default)]
pub(crate) struct LogBuffer(Arc<Mutex<Vec<u8>>>);

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
pub(crate) struct CaptureGuard;

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
pub(crate) fn capture_logs() -> (LogBuffer, CaptureGuard) {
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
