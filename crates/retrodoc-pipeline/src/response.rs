//! Helpers shared by the passes that ask the LLM for a JSON answer.
//!
//! Small or local models often fail to follow "reply with ONLY a JSON
//! object": they add prose, wrap the answer in code fences, emit several
//! fenced blocks, or append trailing text. The helpers here accept the first
//! JSON value found, and [`complete_json`] retries once before giving up.

use retrodoc_llm::{ChatMessage, CompletionRequest, LlmProvider, Role};
use serde::de::DeserializeOwned;

use crate::error::PipelineError;

/// Parses the first JSON value of type `T` found in an LLM answer,
/// tolerating code fences, surrounding prose and trailing content.
pub(crate) fn parse_json_response<T: DeserializeOwned>(raw: &str) -> Result<T, PipelineError> {
    let json = strip_code_fence(raw);
    serde_json::Deserializer::from_str(json)
        .into_iter::<T>()
        .next()
        .unwrap_or_else(|| serde_json::from_str(json))
        .map_err(|source| PipelineError::ResponseParse {
            raw: raw.to_string(),
            source,
        })
}

/// Narrows an LLM answer down to the text where its JSON starts: the body
/// of the first Markdown code fence if there is one (several fenced blocks
/// are common, only the first is kept), otherwise everything from the first
/// `{`.
pub(crate) fn strip_code_fence(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some((_, after_open)) = trimmed.split_once("```") {
        // Skip the language tag (`json`) up to the end of the fence line.
        let body = after_open
            .split_once('\n')
            .map_or(after_open, |(_, rest)| rest);
        return body.split("```").next().unwrap_or(body).trim();
    }
    trimmed.find('{').map_or(trimmed, |start| &trimmed[start..])
}

/// One system + user exchange with the default model, answer as is.
///
/// # Errors
///
/// Returns an error if the LLM call fails.
pub(crate) async fn complete_text(
    llm: &dyn LlmProvider,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, PipelineError> {
    let response = llm
        .complete(CompletionRequest {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: Role::User,
                    content: user_prompt.to_string(),
                },
            ],
            model: None,
        })
        .await?;
    Ok(response.content)
}

/// Asks the LLM for a JSON answer and parses it, retrying once if the
/// answer can't be parsed. `Ok(None)` means both attempts were unparseable
/// (logged as a warning for `what`): the caller skips that unit instead of
/// aborting the run.
///
/// # Errors
///
/// Returns an error only if the LLM call itself fails.
pub(crate) async fn complete_json<T: DeserializeOwned>(
    llm: &dyn LlmProvider,
    system_prompt: &str,
    user_prompt: &str,
    what: &str,
) -> Result<Option<T>, PipelineError> {
    const ATTEMPTS: u32 = 2;
    for attempt in 1..=ATTEMPTS {
        let response = complete_text(llm, system_prompt, user_prompt).await?;
        match parse_json_response(&response) {
            Ok(parsed) => return Ok(Some(parsed)),
            Err(err) => {
                // The error's Display embeds the whole raw answer: keep the
                // warning short and leave the raw text to the debug level.
                let reason = match &err {
                    PipelineError::ResponseParse { source, .. } => source.to_string(),
                    other => other.to_string(),
                };
                tracing::debug!(what, raw = %response, "unparseable LLM response (raw)");
                tracing::warn!(
                    what,
                    attempt,
                    error = %reason,
                    answer_chars = response.chars().count(),
                    "unparseable LLM response{}",
                    if attempt < ATTEMPTS { ", retrying" } else { ", skipped" }
                );
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Answer {
        n: u32,
    }

    #[test]
    fn takes_the_first_fenced_block_and_ignores_the_rest() {
        let raw = "```json\n{\"n\":1}\n```\n```json\n{\"n\":2}\n```";
        assert_eq!(parse_json_response::<Answer>(raw).unwrap(), Answer { n: 1 });
    }

    #[test]
    fn tolerates_prose_and_trailing_content() {
        let raw = "Sure! Here you go: {\"n\":3} hope that helps {oops";
        assert_eq!(parse_json_response::<Answer>(raw).unwrap(), Answer { n: 3 });
    }

    #[test]
    fn tolerates_an_unclosed_fence() {
        let raw = "```json\n{\"n\":4}";
        assert_eq!(parse_json_response::<Answer>(raw).unwrap(), Answer { n: 4 });
    }

    #[test]
    fn rejects_an_answer_without_json() {
        assert!(parse_json_response::<Answer>("I cannot do that").is_err());
    }

    /// Answers with each canned response in turn, counting calls.
    struct SequenceProvider {
        responses: Vec<&'static str>,
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl LlmProvider for SequenceProvider {
        async fn complete(
            &self,
            _: CompletionRequest,
        ) -> Result<retrodoc_llm::CompletionResponse, retrodoc_llm::LlmError> {
            let i = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(retrodoc_llm::CompletionResponse {
                content: self.responses[i.min(self.responses.len() - 1)].to_string(),
                model: "test-model".to_string(),
                ..Default::default()
            })
        }
    }

    #[tokio::test]
    async fn complete_json_retries_once_then_succeeds() {
        let provider = SequenceProvider {
            responses: vec!["garbage", "{\"n\":5}"],
            calls: 0.into(),
        };
        let got = complete_json::<Answer>(&provider, "s", "u", "test")
            .await
            .unwrap();
        assert_eq!(got, Some(Answer { n: 5 }));
        assert_eq!(provider.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn complete_json_gives_up_after_two_attempts() {
        let provider = SequenceProvider {
            responses: vec!["garbage"],
            calls: 0.into(),
        };
        let got = complete_json::<Answer>(&provider, "s", "u", "test")
            .await
            .unwrap();
        assert_eq!(got, None);
        assert_eq!(provider.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
