//! `retrodoc-llm`: LLM provider abstraction (`LlmProvider`), so `RetroDoc`
//! isn't locked to `OpenRouter` (PLAN.md §1). Only `OpenRouter` is
//! implemented in v1. The real HTTP call (PLAN.md §4: "retry, rate-limit")
//! is wired up here, at the "repo map" phase — the first pipeline step to
//! consume an LLM.

mod heartbeat;
mod openrouter;
mod types;
mod usage;

pub use heartbeat::{HeartbeatProvider, DEFAULT_HEARTBEAT};
pub use openrouter::OpenRouterProvider;
pub use types::{ChatMessage, CompletionRequest, CompletionResponse, LlmError, LlmProvider, Role};
pub use usage::{
    CallTotals, PassUsage, Usage, UsageProvider, UsageReport, UsageTracker, UNLABELLED_PASS,
};
