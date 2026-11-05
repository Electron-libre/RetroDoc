---
priority: high
why: 'the ask agent cannot navigate the docs without it; first step of the milestone'
---
# Let an LLM provider call tools

# Goal
`LlmProvider::complete` takes plain text messages and returns text: a model can't ask for a tool, so no agent can
navigate the generated docs by itself (step 2 of `docs/findings.md` §4). Expected: a request can carry tools, an
answer can carry tool calls, and the next request can give their results back, through OpenRouter and DeepSeek,
with the tokens of each round counted like any other call.

# Findings
* 2026-11-05, read from the code: `CompletionRequest` holds `messages`, `model` and `json_schema` only;
  `ChatMessage` has no tool role; `OpenRouterProvider` (also DeepSeek, same wire format) sends no `tools` and reads
  no `tool_calls` (`crates/retrodoc-llm/src/types.rs`, `openrouter/mod.rs`).
* The MCP tools are already plain methods of `retrodoc_mcp::Docs` returning Markdown (`crates/retrodoc-mcp/src/tools/`),
  so an internal agent can call them without the MCP transport.

# Approach
1. Native tool calling: `tools` (name, description, JSON schema of the arguments) in `CompletionRequest`,
   `tool_calls` in `CompletionResponse`, an assistant message carrying its calls and a `tool` message carrying one
   result (OpenAI-compatible format). The wrappers (usage, heartbeat, language) pass them through.
2. A JSON-action fallback over `complete` (the model answers `{"tool": ..., "arguments": ...}` or a final answer)
   for a model or a server without native tools, the local ones above all.
3. Decide between 1, 2 or both (1 with 2 as fallback) in an ADR, from what OpenRouter, DeepSeek and the local
   llama-server/Ollama accept, checked on a real call before the paid runs.

# Specs
* A new spec of the tool calls of a provider (e.g. `specs/tool-calling.md`), written at `issue-workflow` step 1.

# Milestone
Ask the documentation

# Resources
* `docs/findings.md` §4 (design of the agent), `docs/crates/llm.md`, `crates/retrodoc-llm/src/types.rs`,
  `crates/retrodoc-llm/src/openrouter/`, `crates/retrodoc-llm/src/usage.rs`.
* ADR 0016 (the MCP server is LLM-free: the agent is a separate consumer), ADR 0041 (the language wrapper).

# Hints
* Structured output (`json_schema`) and tools together: some servers refuse both in one request; the agent's
  final answer may need a separate shape.
* The fake provider of the tests must script tool calls, like the pipeline passes script their answers.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
A request can offer tools, an answer can carry tool calls, the next request gives the results back, in the native
OpenAI-compatible format, through every wrapper and counted per round. Out of scope: the agent loop (`ask`) and the
JSON-action fallback (ADR 0046: the local Ollama takes native tools, checked on a real call).

## Plan
1. [x] Native tool calls: `ToolSpec`, `ToolCall`, `Role::Tool` and the call fields of `ChatMessage`, `tools` of
   `CompletionRequest`, `tool_calls` of `CompletionResponse`, sent and read by `OpenRouterProvider`; verified by
   wire tests on the mock server and by wrapper tests; makes true TCL-1 to TCL-9. (autonomous)
2. [x] `FakeLlm` scripts tool calls and records the full messages of each request; verified by its own test; makes
   true TCL-10. (autonomous)
3. [x] Findings of the `high` review: a `null` `tool_calls`, a message with no text nor call, `null` arguments and a
   missing id read safely; `FakeLlm` records the tools offered and scripts a text next to the calls; verified by wire
   and fake tests; makes true TCL-11 to TCL-14. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: c907cccad20742a01bfe771e6ceb961645486634
- Real calls before the ADR (1 a): DeepSeek (`deepseek-chat`, served as `deepseek-flash`) and Ollama both answer
  `tool_calls` and take the `tool` message back; DeepSeek writes text next to its calls, so an assistant message
  keeps its text with its calls. Ledger line of 2026-11-05.
- `reasoning_content` (2 a): not needed, the runs use `deepseek-chat` (no thinking mode); not carried.
- llama-server hybrid not probed (3 a); no JSON-action fallback (4 a).

## Deliverable 1 summary
- done: `ToolSpec`, `ToolCall`, `Role::Tool`, `ChatMessage::{new, assistant_calls, tool_result}`,
  `CompletionRequest.tools` (now `Default`), `CompletionResponse.tool_calls`; `OpenRouterProvider` sends `tools`, the
  calls and results of the history, reads `tool_calls` and a `null` content; ADR 0046, `specs/tool-calling.md`, docs.
- checks: tests, clippy, fmt, specs (`project-check`) pass; docs updated (ARCHITECTURE §8, `docs/crates/llm.md`,
  findings §4, PRODUCT open question removed).
- red: TCL-1, TCL-3, TCL-4, TCL-5, TCL-6, TCL-7: `tools` absent from the body (`left: Null`), `error decoding response
  body` on a `null` content, the history sent without `tool_calls` nor `tool_call_id`. TCL-2, TCL-8, TCL-9 green from
  the start (no field sent before; the wrappers already pass the request and the answer through whole).
- Code review: no findings
- not verified: `OpenRouter` and the hybrid llama-server not called for real (user's choice, 3 a).

## Deliverable 2 summary
- done: `FakeLlm::scripted` answers a script of `FakeTurn::Calls` / `FakeTurn::Text`, `FakeLlm::messages` keeps the
  messages of each request; spec `accepted` (every requirement traced), ADR 0046 `Accepted`; conventions doc.
- checks: `project-check` passes (tests, clippy, fmt, specs).
- Code review: no findings
- red: TCL-10: `no associated function or constant named scripted found for struct FakeLlm`.
- not verified: nothing.

## Deliverable 3 summary
- done: `tool_calls: null` read as none (TCL-11), no text and no call an invalid response again (TCL-12), `null`
  arguments are `{}` (TCL-13), a call without an id gets a distinct one (TCL-14); `FakeLlm` records the tools offered
  and `FakeTurn::Calls` carries a text; the language test uses `FakeLlm`. New issue
  `issues/done/tool_loop_on_thinking_and_refusing_servers.md` for the `reasoning_content` of DeepSeek thinking mode and
  the fallback loop on a refusal of tools.
- checks: `project-check` passes.
- Code review: no findings
- red: TCL-11, TCL-12, TCL-13, TCL-14: `error decoding response body` on `tool_calls: null`, `Ok` with an empty text
  for `content: null`, arguments `["", ""]`, ids `{"", "call_0"}`.
- high review: 9 findings; fixed 1 (`null` `tool_calls`), 2 (`null` content without calls), 3 (`null` arguments),
  6 (missing id), 7 (`FakeLlm` records tools), 9 (text next to calls); 4 (`reasoning_content`) and 5 (fallback on a
  refusal of tools) to the new issue; 8 not fixed: `prompts` stays for its many callers, the clone of each history is
  a test fake's cost on short loops.
- not verified: nothing new.
