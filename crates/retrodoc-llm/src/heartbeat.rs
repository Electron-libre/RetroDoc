//! A provider decorator that says a call is still pending.

use std::time::Duration;

use async_trait::async_trait;

use crate::{CompletionRequest, CompletionResponse, LlmError, LlmProvider};

/// Interval of the "still waiting" log of [`HeartbeatProvider::new`].
pub const DEFAULT_HEARTBEAT: Duration = Duration::from_secs(30);

/// Wraps a provider to report, at a regular interval, that a call is still
/// pending (including the retries and backoff inside it), so a slow call can
/// be told from a stuck run in the logs.
pub struct HeartbeatProvider<P> {
    inner: P,
    interval: Duration,
    report: Box<dyn Fn(Duration) + Send + Sync>,
}

impl<P> HeartbeatProvider<P> {
    /// Logs "still waiting for the LLM" every [`DEFAULT_HEARTBEAT`].
    #[must_use]
    pub fn new(inner: P) -> Self {
        Self::with_reporter(inner, DEFAULT_HEARTBEAT, |waited| {
            tracing::info!("still waiting for the LLM ({}s)", waited.as_secs());
        })
    }

    /// `report` is called every `interval` with the time waited so far.
    #[must_use]
    pub fn with_reporter(
        inner: P,
        interval: Duration,
        report: impl Fn(Duration) + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner,
            interval,
            report: Box::new(report),
        }
    }
}

#[async_trait]
impl<P: LlmProvider> LlmProvider for HeartbeatProvider<P> {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let started = tokio::time::Instant::now();
        let call = self.inner.complete(request);
        tokio::pin!(call);
        let mut ticker = tokio::time::interval_at(started + self.interval, self.interval);
        loop {
            tokio::select! {
                result = &mut call => return result,
                _ = ticker.tick() => (self.report)(started.elapsed()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SlowProvider(Duration);

    #[async_trait]
    impl LlmProvider for SlowProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            tokio::time::sleep(self.0).await;
            Ok(CompletionResponse {
                content: "done".to_string(),
                model: "m".to_string(),
                ..Default::default()
            })
        }
    }

    fn request() -> CompletionRequest {
        CompletionRequest {
            messages: Vec::new(),
            model: None,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn heartbeat_reports_while_a_call_is_pending_and_passes_the_answer_through() {
        let ticks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = ticks.clone();
        let provider = HeartbeatProvider::with_reporter(
            SlowProvider(Duration::from_secs(100)),
            Duration::from_secs(30),
            move |waited| seen.lock().unwrap().push(waited.as_secs()),
        );

        let response = provider.complete(request()).await.unwrap();

        assert_eq!(response.content, "done");
        assert_eq!(*ticks.lock().unwrap(), vec![30, 60, 90]);
    }

    #[tokio::test(start_paused = true)]
    async fn heartbeat_is_silent_for_a_fast_call() {
        let ticks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = ticks.clone();
        let provider = HeartbeatProvider::with_reporter(
            SlowProvider(Duration::from_secs(5)),
            Duration::from_secs(30),
            move |waited| seen.lock().unwrap().push(waited.as_secs()),
        );
        provider.complete(request()).await.unwrap();
        assert!(ticks.lock().unwrap().is_empty());
    }
}
