# 0002. `LlmProvider` trait, OpenRouter as the only provider

Status: Accepted

_Retroactive ADR, reconstructed from the history (71b8e98, 86068f5, c664ac1; 2026-09-28 to 2026-09-30)._

## Context

v1 needs one LLM backend, but the model choice (quality vs. cost) is an open risk in `PLAN.md` §6 and
must not be locked in. Smoke tests also need a local model (Ollama, LM Studio, llama.cpp) that speaks
the same chat-completions wire format.

## Decision

All passes talk to a `LlmProvider` trait in `retrodoc-llm`. The only implementation is
`OpenRouterProvider`, a `reqwest` client. `llm.base_url` overrides the endpoint so any server with the
same wire format works; auth and client stay OpenRouter's, so this is not multi-provider support (still
out of scope for v1). `llm.reasoning_effort` is an optional pass-through, and a `finish_reason=length`
answer logs a warning. Per-provider adapters (Anthropic, OpenAI native APIs) were rejected as out of
scope for v1.

## Consequences

Local and free-tier models can be used for smoke tests without code changes. Provider-specific
features (native structured output, token accounting) must go through the trait or be absent. A real
second provider will need a new ADR.
