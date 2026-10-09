//! Token accounting: what a completion cost, as the server reports it.
//! Nothing here estimates tokens; a call whose server sent no (or an
//! incomplete) `usage` block simply has no [`Usage`] and is counted apart.
//!
//! [`UsageProvider`] wraps a provider and feeds a shared [`UsageTracker`];
//! the caller names the current pass with [`UsageTracker::set_pass`] and
//! reads the [`UsageReport`] at the end. Only answered calls are counted: a
//! retry inside the wrapped provider is invisible here, and a failed
//! attempt reports no tokens, so the figures are a lower bound when a
//! server is flaky.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::time::Instant;

use crate::{CompletionRequest, CompletionResponse, LlmError, LlmProvider};

/// Token counts of one completion, as reported by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// The `usage` block of an OpenAI-compatible chat completion. Both counters
/// are optional so an incomplete block parses; it is then dropped by
/// [`ApiUsage::into_usage`].
#[derive(Debug, Deserialize)]
struct ApiUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
}

impl ApiUsage {
    fn into_usage(self) -> Option<Usage> {
        Some(Usage {
            prompt_tokens: self.prompt_tokens?,
            completion_tokens: self.completion_tokens?,
        })
    }
}

/// Reads the raw `usage` value of a response. Accounting must never cost a
/// valid answer: a block that is missing, incomplete or malformed (float,
/// negative or textual counters) is `None`, not a parse error.
pub(crate) fn parse(raw: Option<serde_json::Value>) -> Option<Usage> {
    // Serde would read a JSON array as the struct's fields in order.
    let raw = raw.filter(serde_json::Value::is_object)?;
    serde_json::from_value::<ApiUsage>(raw).ok()?.into_usage()
}

/// Name of the pass that calls made before any [`UsageTracker::set_pass`]
/// are attributed to.
pub const UNLABELLED_PASS: &str = "unlabelled";

/// What a set of calls cost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CallTotals {
    /// Answered calls, with or without a `usage` block.
    pub calls: u64,
    /// Among `calls`, those whose server reported no usable `usage`: their
    /// tokens are missing from the sums below, not estimated.
    pub calls_without_usage: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

impl CallTotals {
    fn record(&mut self, usage: Option<Usage>) {
        self.calls += 1;
        match usage {
            Some(usage) => {
                self.prompt_tokens += usage.prompt_tokens;
                self.completion_tokens += usage.completion_tokens;
            }
            None => self.calls_without_usage += 1,
        }
    }

    fn add(&mut self, other: &CallTotals) {
        self.calls += other.calls;
        self.calls_without_usage += other.calls_without_usage;
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
    }
}

/// One pass of the pipeline: its wall-clock time (from its first
/// [`UsageTracker::set_pass`] to the next pass, summed if it is entered
/// several times) and its calls per model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassUsage {
    pub name: String,
    pub wall: Duration,
    pub models: BTreeMap<String, CallTotals>,
    /// Answers the pass could not parse (each one costs a retry, or a unit).
    pub unparseable: u64,
    /// Among `unparseable`, those after which the pass gave up on the unit.
    pub skipped: u64,
}

impl PassUsage {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            wall: Duration::ZERO,
            models: BTreeMap::new(),
            unparseable: 0,
            skipped: 0,
        }
    }

    /// All models of the pass together.
    #[must_use]
    pub fn total(&self) -> CallTotals {
        let mut total = CallTotals::default();
        for models in self.models.values() {
            total.add(models);
        }
        total
    }
}

/// Usage of a run, passes in the order they were first entered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageReport {
    pub passes: Vec<PassUsage>,
}

impl UsageReport {
    /// All passes and models together.
    #[must_use]
    pub fn total(&self) -> CallTotals {
        let mut total = CallTotals::default();
        for pass in &self.passes {
            total.add(&pass.total());
        }
        total
    }

    /// Wall-clock time of all passes.
    #[must_use]
    pub fn wall(&self) -> Duration {
        self.passes.iter().map(|p| p.wall).sum()
    }
}

#[derive(Default)]
struct Inner {
    passes: Vec<PassUsage>,
    /// Index of the open pass and when it was entered or last measured.
    current: Option<(usize, Instant)>,
}

impl Inner {
    fn close_current(&mut self, now: Instant) {
        if let Some((index, since)) = self.current.take() {
            self.passes[index].wall += now.saturating_duration_since(since);
        }
    }

    fn enter(&mut self, name: &str, now: Instant) -> usize {
        self.close_current(now);
        let index = self
            .passes
            .iter()
            .position(|p| p.name == name)
            .unwrap_or_else(|| {
                self.passes.push(PassUsage::new(name));
                self.passes.len() - 1
            });
        self.current = Some((index, now));
        index
    }
}

/// Shared counters of a run. Clones share the same state, so the wrapper
/// given to the pipeline and the caller reading the report see the same
/// numbers; the lock is never held across an await.
#[derive(Clone, Default)]
pub struct UsageTracker {
    inner: Arc<Mutex<Inner>>,
}

impl UsageTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Names the pass the following calls belong to and closes the previous
    /// one. Entering a name already seen resumes it.
    pub fn set_pass(&self, name: &str) {
        self.lock().enter(name, Instant::now());
    }

    /// Closes the current pass: the time that follows (writing the docs, say)
    /// belongs to none, and calls recorded after it go to
    /// [`UNLABELLED_PASS`].
    pub fn end_pass(&self) {
        self.lock().close_current(Instant::now());
    }

    /// Runs `work` as the pass `name`, and closes the pass when it ends, so
    /// whatever follows (printing, writing the docs) is billed to none.
    pub async fn in_pass<T>(&self, name: &str, work: impl Future<Output = T>) -> T {
        self.set_pass(name);
        let result = work.await;
        self.end_pass();
        result
    }

    /// Counts one answered call in the current pass.
    pub fn record(&self, model: &str, usage: Option<Usage>) {
        let mut inner = self.lock();
        let index = match inner.current {
            Some((index, _)) => index,
            None => inner.enter(UNLABELLED_PASS, Instant::now()),
        };
        inner.passes[index]
            .models
            .entry(model.to_string())
            .or_default()
            .record(usage);
    }

    /// Counts one unparseable answer in the current pass.
    pub fn record_unparseable(&self, skipped: bool) {
        let mut inner = self.lock();
        let index = match inner.current {
            Some((index, _)) => index,
            None => inner.enter(UNLABELLED_PASS, Instant::now()),
        };
        let pass = &mut inner.passes[index];
        pass.unparseable += 1;
        pass.skipped += u64::from(skipped);
    }

    /// What was counted so far; the open pass is measured up to now.
    #[must_use]
    pub fn report(&self) -> UsageReport {
        let inner = self.lock();
        let mut passes = inner.passes.clone();
        if let Some((index, since)) = inner.current {
            passes[index].wall += Instant::now().saturating_duration_since(since);
        }
        UsageReport { passes }
    }
}

/// Wraps a provider to count what its answered calls cost in a
/// [`UsageTracker`]. Answers and errors pass through unchanged.
pub struct UsageProvider<P> {
    inner: P,
    tracker: UsageTracker,
}

impl<P> UsageProvider<P> {
    #[must_use]
    pub fn new(inner: P, tracker: UsageTracker) -> Self {
        Self { inner, tracker }
    }
}

#[async_trait]
impl<P: LlmProvider> LlmProvider for UsageProvider<P> {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let response = self.inner.complete(request).await?;
        self.tracker.record(&response.model, response.usage);
        Ok(response)
    }

    fn note_unparseable_answer(&self, skipped: bool) {
        self.tracker.record_unparseable(skipped);
        self.inner.note_unparseable_answer(skipped);
    }

    fn model(&self) -> &str {
        self.inner.model()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Answers with a fixed model and usage, or fails when `fail` is set.
    struct FakeProvider {
        model: &'static str,
        usage: Option<Usage>,
        fail: bool,
    }

    impl FakeProvider {
        fn answering(model: &'static str, prompt: u64, completion: u64) -> Self {
            Self {
                model,
                usage: Some(Usage {
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                }),
                fail: false,
            }
        }
    }

    #[async_trait]
    impl LlmProvider for FakeProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            if self.fail {
                return Err(LlmError::Transport("down".to_string()));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(CompletionResponse {
                content: "ok".to_string(),
                model: self.model.to_string(),
                usage: self.usage,
            })
        }
    }

    fn request() -> CompletionRequest {
        CompletionRequest {
            messages: Vec::new(),
            model: None,
            json_schema: None,
        }
    }

    fn totals(calls: u64, without: u64, prompt: u64, completion: u64) -> CallTotals {
        CallTotals {
            calls,
            calls_without_usage: without,
            prompt_tokens: prompt,
            completion_tokens: completion,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn calls_are_counted_per_pass_and_per_model() {
        let tracker = UsageTracker::new();
        let big = UsageProvider::new(FakeProvider::answering("big", 100, 20), tracker.clone());
        let small = UsageProvider::new(FakeProvider::answering("small", 10, 2), tracker.clone());

        tracker.set_pass("features");
        big.complete(request()).await.unwrap();
        big.complete(request()).await.unwrap();
        small.complete(request()).await.unwrap();
        tracker.set_pass("use-cases");
        big.complete(request()).await.unwrap();

        let report = tracker.report();
        let names: Vec<_> = report.passes.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["features", "use-cases"]);
        assert_eq!(report.passes[0].models["big"], totals(2, 0, 200, 40));
        assert_eq!(report.passes[0].models["small"], totals(1, 0, 10, 2));
        assert_eq!(report.passes[0].total(), totals(3, 0, 210, 42));
        assert_eq!(report.passes[1].total(), totals(1, 0, 100, 20));
        assert_eq!(report.total(), totals(4, 0, 310, 62));
    }

    #[tokio::test(start_paused = true)]
    async fn a_call_without_usage_is_counted_but_adds_no_tokens() {
        let tracker = UsageTracker::new();
        let silent = UsageProvider::new(
            FakeProvider {
                model: "local",
                usage: None,
                fail: false,
            },
            tracker.clone(),
        );
        let loud = UsageProvider::new(FakeProvider::answering("local", 7, 3), tracker.clone());
        tracker.set_pass("roles");
        silent.complete(request()).await.unwrap();
        loud.complete(request()).await.unwrap();

        assert_eq!(tracker.report().total(), totals(2, 1, 7, 3));
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_call_is_not_counted_and_the_error_passes_through() {
        let tracker = UsageTracker::new();
        let down = UsageProvider::new(
            FakeProvider {
                model: "m",
                usage: None,
                fail: true,
            },
            tracker.clone(),
        );
        tracker.set_pass("roles");
        let result = down.complete(request()).await;

        assert!(matches!(result, Err(LlmError::Transport(_))));
        assert_eq!(tracker.report().total(), CallTotals::default());
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_calls_all_land_in_the_pass() {
        let tracker = UsageTracker::new();
        let provider = std::sync::Arc::new(UsageProvider::new(
            FakeProvider::answering("m", 5, 1),
            tracker.clone(),
        ));
        tracker.set_pass("repo-map");

        let mut calls = tokio::task::JoinSet::new();
        for _ in 0..64 {
            let provider = provider.clone();
            calls.spawn(async move { provider.complete(request()).await });
        }
        while let Some(result) = calls.join_next().await {
            result.unwrap().unwrap();
        }

        let report = tracker.report();
        assert_eq!(report.passes.len(), 1);
        assert_eq!(report.total(), totals(64, 0, 320, 64));
        // 64 calls of 10 ms ran together, not one after the other.
        assert_eq!(report.passes[0].wall, Duration::from_millis(10));
    }

    #[tokio::test(start_paused = true)]
    async fn wall_time_is_measured_per_pass_and_a_pass_can_be_resumed() {
        let tracker = UsageTracker::new();
        tracker.set_pass("a");
        tokio::time::sleep(Duration::from_secs(5)).await;
        tracker.set_pass("b");
        tokio::time::sleep(Duration::from_secs(2)).await;
        tracker.set_pass("a");
        tokio::time::sleep(Duration::from_secs(1)).await;

        let report = tracker.report();
        assert_eq!(report.passes.len(), 2);
        assert_eq!(report.passes[0].wall, Duration::from_secs(6));
        assert_eq!(report.passes[1].wall, Duration::from_secs(2));
        assert_eq!(report.wall(), Duration::from_secs(8));
        // Reading the report does not close the open pass.
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(tracker.report().passes[0].wall, Duration::from_secs(7));
    }

    #[tokio::test(start_paused = true)]
    async fn an_ended_pass_stops_its_clock() {
        let tracker = UsageTracker::new();
        tracker.set_pass("a");
        tokio::time::sleep(Duration::from_secs(3)).await;
        tracker.end_pass();
        tokio::time::sleep(Duration::from_secs(10)).await;

        assert_eq!(tracker.report().passes[0].wall, Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn a_pass_run_with_in_pass_is_closed_when_its_work_ends() {
        let tracker = UsageTracker::new();
        let provider = UsageProvider::new(FakeProvider::answering("m", 1, 1), tracker.clone());

        tracker
            .in_pass("a", async {
                tokio::time::sleep(Duration::from_secs(3)).await;
                provider.complete(request()).await.unwrap();
            })
            .await;
        tokio::time::sleep(Duration::from_secs(10)).await;
        provider.complete(request()).await.unwrap();

        let report = tracker.report();
        assert_eq!(report.passes[0].name, "a");
        // The 10 s that follow are not in it (the paused clock may add a tick).
        assert!(report.passes[0].wall >= Duration::from_secs(3));
        assert!(report.passes[0].wall < Duration::from_secs(4));
        assert_eq!(report.passes[0].total(), totals(1, 0, 1, 1));
        // A call outside any pass is not billed to the last one.
        assert_eq!(report.passes[1].name, UNLABELLED_PASS);
    }

    #[tokio::test(start_paused = true)]
    async fn calls_before_any_pass_are_not_lost() {
        let tracker = UsageTracker::new();
        let provider = UsageProvider::new(FakeProvider::answering("m", 1, 1), tracker.clone());
        provider.complete(request()).await.unwrap();

        let report = tracker.report();
        assert_eq!(report.passes[0].name, UNLABELLED_PASS);
        assert_eq!(report.total(), totals(1, 0, 1, 1));
    }

    fn parse(json: &str) -> Option<Usage> {
        super::parse(Some(serde_json::from_str(json).unwrap()))
    }

    #[test]
    fn both_counters_make_a_usage() {
        assert_eq!(
            parse(r#"{"prompt_tokens":120,"completion_tokens":35,"total_tokens":155}"#),
            Some(Usage {
                prompt_tokens: 120,
                completion_tokens: 35
            })
        );
    }

    #[test]
    fn a_malformed_block_is_absent_not_an_error() {
        for json in [
            r#"{"prompt_tokens":12.5,"completion_tokens":3}"#,
            r#"{"prompt_tokens":-1,"completion_tokens":3}"#,
            r#"{"prompt_tokens":"12","completion_tokens":3}"#,
            r#""12 tokens""#,
            "[1,2]",
        ] {
            assert_eq!(parse(json), None, "{json}");
        }
        assert_eq!(super::parse(None), None);
    }

    #[test]
    fn a_block_missing_a_counter_is_not_trusted() {
        // Some local servers send `usage: {}` or null counters; treat the
        // whole block as absent rather than counting a half-known call.
        for json in [
            "{}",
            r#"{"prompt_tokens":10}"#,
            r#"{"completion_tokens":10}"#,
            r#"{"prompt_tokens":null,"completion_tokens":3}"#,
            "null",
        ] {
            assert_eq!(parse(json), None, "{json}");
        }
    }

    #[tokio::test]
    async fn unparseable_answers_are_counted_in_the_current_pass_through_the_wrapper() {
        let tracker = UsageTracker::new();
        let provider = UsageProvider::new(FakeProvider::answering("m", 1, 1), tracker.clone());
        tracker.set_pass("features");
        provider.note_unparseable_answer(false);
        provider.note_unparseable_answer(true);
        tracker.set_pass("use cases");
        provider.note_unparseable_answer(false);

        let report = tracker.report();
        assert_eq!(
            (report.passes[0].unparseable, report.passes[0].skipped),
            (2, 1)
        );
        assert_eq!(
            (report.passes[1].unparseable, report.passes[1].skipped),
            (1, 0)
        );
    }
}
