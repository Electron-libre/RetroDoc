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
