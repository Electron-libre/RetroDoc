---
priority: medium
why: 'the ask agent works on deepseek-chat and the local Ollama without it; a thinking model or a server without tools breaks or slows the loop'
---
# Run a tool loop on a thinking model and on a server that refuses tools

# Goal
The native tool calls of `retrodoc-llm` (ADR 0046, `specs/tool-calling.md`) were checked on `deepseek-chat` (no
thinking) and the local Ollama only. Two cases left out make the agent of `retrodoc ask` fail or wait: a thinking
model whose reasoning must be given back with its calls, and a model or server that refuses tools. Expected: a
tool loop runs on a DeepSeek thinking model, and a refusal of the tools surfaces at once with a clear error.

# Findings
* 2026-11-05, `high` review of `issues/tool_calling_in_llm.md`, read from the code: `ApiResponseMessage`
  (`crates/retrodoc-llm/src/openrouter/mod.rs`) drops `reasoning_content` and `ChatMessage` cannot carry it. The
  DeepSeek docs ask, in thinking mode, the `reasoning_content` of an assistant turn that called tools to be sent back
  within the same question; not checked on a real call (only `deepseek-chat` was, 2026-11-05, ledger).
* Same review: a 400/404/422 caused by the tools (Ollama "does not support tools" on a model without them,
  OpenRouter 404 "no endpoints found that support tool use") is a candidate refused parameter for the temperature and
  schema fallback of `OpenRouterProvider::complete`: up to 3 more requests, each up to the 900 s local timeout, before
  the error, which then names neither the tools nor the model.

# Approach
1. Check on one real call (cents, ask first) whether a DeepSeek thinking model refuses the second round without
   `reasoning_content`; if it does, carry it: an optional reasoning in `CompletionResponse` and in the assistant
   message of `ChatMessage::assistant_calls`, sent back as `reasoning_content`.
2. Skip the temperature/schema fallback when the request offers tools and the error body names the tools, or
   whenever it offers tools and sets neither; make the error say the model or server refuses tools.
3. Both, as two deliverables.

# Specs
* `specs/tool-calling.md`: new requirements (reasoning given back, refusal of tools reported without a fallback).

# Milestone
Ask the documentation

# Resources
* ADR 0046, `specs/tool-calling.md`, `docs/crates/llm.md`, `crates/retrodoc-llm/src/openrouter/mod.rs`
  (`complete`, `Failure::may_be_a_refused_parameter`), `crates/retrodoc-llm/src/openrouter/tests/fallbacks.rs`.

# Hints
* `reasoning_content` must not be sent back for a model that never sent one: other servers may refuse the field.
* The fallback also serves passes without tools: keep its behaviour for them (ADR 0023).
