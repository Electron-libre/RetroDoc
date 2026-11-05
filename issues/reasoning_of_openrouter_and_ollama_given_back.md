---
priority: low
why: 'the ask agent runs on DeepSeek, whose reasoning is given back; a thinking model through OpenRouter or Ollama is a comfort'
---
# Give back the reasoning of a thinking model reached through OpenRouter or Ollama

# Goal
`retrodoc-llm` reads and sends back only the `reasoning_content` of DeepSeek (TCL-17, TCL-18). A thinking model
reached through OpenRouter (the default provider) or a local Ollama loses its reasoning between the rounds of a tool
loop. Expected: the reasoning these servers send is given back in the form each expects, so a model that needs it
(a thought signature, say) runs the agent of `retrodoc ask`.

# Findings
* 2026-11-05, `high` review of `issues/done/tool_loop_on_thinking_and_refusing_servers.md`, read from the code, not
  checked on a real call: OpenRouter returns the reasoning as `reasoning` and `reasoning_details`, Ollama's
  OpenAI-compatible endpoint as `reasoning`; `ApiResponseMessage` (`crates/retrodoc-llm/src/openrouter/mod.rs`)
  reads neither.

# Approach
1. Check on one real call per server (OpenRouter: cents, ask first; Ollama: local) which field comes back and
   whether a round without it is refused or degraded.
2. Read the fields found, keep them with the reasoning of `CompletionResponse`, and send each back in the field its
   server sent (never one a server did not send).

# Specs
* `specs/tool-calling.md`: amend TCL-17 and TCL-18 (fields other than `reasoning_content`).

# Milestone
Ask the documentation

# Resources
* `specs/tool-calling.md`, `docs/crates/llm.md`, ADR 0046, `crates/retrodoc-llm/src/openrouter/mod.rs`,
  `crates/retrodoc-llm/src/openrouter/tests/tools.rs`.

# Hints
* `reasoning_details` is a list of typed objects (encrypted ones included): give it back as received, not as text.
