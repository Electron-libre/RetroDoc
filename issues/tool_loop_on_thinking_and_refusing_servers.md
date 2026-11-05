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
* 2026-11-05, `high` review of `issues/done/tool_calling_in_llm.md`, read from the code: `ApiResponseMessage`
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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
A refusal of the tools fails at once with an error naming the tools and the model, without the temperature/schema
fallback (kept as is for every other failure); the `reasoning_content` of a thinking model is read and sent back
with its calls. Out of scope: a text protocol for servers without tools (ADR 0046), streaming of the reasoning.

## Plan
1. [x] Refusal of tools: `complete` skips the fallback when the request offers tools and the 400/404/422 body names
   the tools, and the error says the model or server refuses tools; verified by fake-server tests in
   `openrouter/tests/fallbacks.rs`; makes true TCL-15, TCL-16. (autonomous)
2. [ ] Reasoning given back: an optional `reasoning` in `CompletionResponse` and `ChatMessage`, read from
   `reasoning_content`, sent back only when set, carried by the agent of `ask`; verified by fake-server tests in
   `openrouter/tests/tools.rs` and a fake-provider test of the agent, then one real tool loop on a DeepSeek thinking
   model (cents); makes true TCL-17, TCL-18, TCL-19.

## Deliverable 1

- done: a 400/404/422 naming the tools to a request offering tools fails at once, without the fallback, with an
  error naming the model and saying it refuses tools; the fallback is unchanged otherwise.
- red: TCL-15, TCL-16: `called Result::unwrap_err() on an Ok value` (the fallback without the temperature succeeded).
- Code review: 1 finding: a body naming the tools and a parameter (the pair refused) skipped the fallback; fixed,
  such a body is left to the fallback (test with `response_format is not supported together with tools`).
- not verified: on a real Ollama or OpenRouter refusal (bodies taken from the issue's findings).

## Decisions
- Mode: autonomous
- Base commit: ace08063e1bee8d7be5550ed2bc846ab4df4d237
- Real check (1 a): one real tool loop after deliverable 2, not a probe before it.
- Model of the real check (2 a): `deepseek-reasoner`.
