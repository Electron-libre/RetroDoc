# 0046. Tool calls use the native OpenAI-compatible format

Status: Proposed

## Context

The agent of the milestone "Ask the documentation" navigates the generated docs with tools (`docs/findings.md` §4).
`LlmProvider::complete` only takes and returns text. Two ways exist: the native `tools` / `tool_calls` of the
OpenAI-compatible chat completions, or a JSON-action protocol over plain text (the model answers
`{"tool": ..., "arguments": ...}`), which any server takes but which the code must prompt, parse and repair itself.

Checked on real calls (2026-11-05): the local Ollama (`qwen3.6:35b-a3b`, OpenAI-compatible endpoint) and `DeepSeek`
(`deepseek-chat`) answer a request with `tools` by a `tool_calls` message (`finish_reason: tool_calls`, usage
reported; Ollama with an empty text, `DeepSeek` with a sentence next to the calls) and take the `assistant` + `tool`
messages of the next round. `OpenRouter` documents the same format; the hybrid llama-server needs its `--jinja` flag.

## Decision

`retrodoc-llm` speaks the native format only: `CompletionRequest.tools`, `CompletionResponse.tool_calls`, an
assistant message carrying its calls and a `tool` message carrying one result (`specs/tool-calling.md`). The arguments
are handed over as the raw text the server sent; the caller parses them. No JSON-action fallback is built now.

Rejected: the JSON-action fallback alone (every server takes it, but it costs prompt tokens, its parsing fails where
the server's grammar would not, and the servers we use all accept native tools); both (a second path to keep and test
for a server none of our setups needs; it can be added behind the same types if one does).

## Consequences

A model or server without native tools cannot run the agent; `OpenRouter` routes to a server that takes them when the
model allows it. The existing fallbacks (schema, temperature dropped after a 400/404/422) stay as they are; a request
with both tools and a schema relies on them when a server refuses the pair.
