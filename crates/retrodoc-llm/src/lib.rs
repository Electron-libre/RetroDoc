//! `retrodoc-llm`: LLM provider abstraction (`LlmProvider`), so `RetroDoc`
//! isn't locked to `OpenRouter` (PRODUCT.md "Scope of v1"). Only `OpenRouter` is
//! implemented in v1. The real HTTP call (docs/ARCHITECTURE.md §8: "retry, rate-limit")
//! is wired up here.

mod heartbeat;
mod openrouter;
mod types;
mod usage;

pub use heartbeat::{HeartbeatProvider, DEFAULT_HEARTBEAT};
pub use openrouter::OpenRouterProvider;
pub use types::{
    ChatMessage, CompletionRequest, CompletionResponse, LlmError, LlmProvider, ResponseSchema, Role,
};
pub use usage::{
    CallTotals, PassUsage, Usage, UsageProvider, UsageReport, UsageTracker, UNLABELLED_PASS,
};
