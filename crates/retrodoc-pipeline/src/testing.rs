//! A fake [`LlmProvider`] for the tests of the passes, so that none of them
//! needs a network, nor its own copy of the same few lines.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use retrodoc_llm::{CompletionRequest, CompletionResponse, LlmError, LlmProvider};

type Reply = Box<dyn Fn(usize, &CompletionRequest) -> Result<String, LlmError> + Send + Sync>;

/// Answers with what its reply function says, counts its calls and keeps the
/// prompts it receives.
pub(crate) struct FakeLlm {
    reply: Reply,
    calls: AtomicUsize,
    prompts: Mutex<Vec<(String, String)>>,
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
        }
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

    /// The (system, user) prompts of each call received.
    pub fn prompt_pairs(&self) -> Vec<(String, String)> {
        self.prompts.lock().unwrap().clone()
    }
}

#[async_trait]
impl LlmProvider for FakeLlm {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let prompt = |i: usize| {
            request
                .messages
                .get(i)
                .map(|m| m.content.clone())
                .unwrap_or_default()
        };
        self.prompts.lock().unwrap().push((prompt(0), prompt(1)));
        let content = (self.reply)(n, &request)?;
        Ok(CompletionResponse {
            content,
            model: "test-model".to_string(),
            ..Default::default()
        })
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
