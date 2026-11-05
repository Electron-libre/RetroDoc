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
* 2026-11-06, local probe, Ollama 0.40.7 on :11435, `qwen3.6:35b-a3b` (thinking + tools), `/v1/chat/completions`:
  the answer to a tool round holds `reasoning` (no `reasoning_content`, no `reasoning_details`). The next round with
  `reasoning` given back is accepted (HTTP 200) and the reasoning is read (1,843 prompt tokens against 324 without);
  without it, the round is not refused either. OpenRouter not probed: no key on the machine.

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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
Keep every reasoning field a server sends (`reasoning_content` of DeepSeek, `reasoning` of Ollama and OpenRouter,
`reasoning_details` of OpenRouter, the latter as raw JSON) and give each back in its own field, none other. Out of
scope: showing the reasoning to the user, asking for it (`reasoning_effort` stays as is).

## Plan
1. [x] Reasoning fields kept and given back: `Reasoning` struct (`reasoning_content`, `reasoning`,
   `reasoning_details` as `serde_json::Value`) replaces `Option<String>` in `CompletionResponse` and `ChatMessage`;
   read in `ApiResponseMessage`, sent by `ApiMessage` only when present; agent and fake provider follow; verified by
   tests in `openrouter/tests/tools.rs` (OpenRouter-shaped and Ollama-shaped answers, round trip) and the agent test;
   makes true TCL-17, TCL-18, TCL-20 (~250 lines). (autonomous)
2. [x] Fixes of the `high` review: the reasoning fields declared once (serde on `Reasoning`, flattened in the wire
   messages), `of_answer` crate-private, the fake provider scripts any `Reasoning` so the agent test covers
   `reasoning` and `reasoning_details`, a test of empty fields sent back; verified by the agent test and
   `openrouter/tests/tools.rs`; makes true TCL-18, TCL-19, TCL-20 (~80 lines). (autonomous)

## Decisions
- Mode: autonomous
- `high` review findings 1 to 3 (a refused foreign `reasoning_details`, both fields sent back, the reasoning outside
  the agent's budget) need a real OpenRouter call: follow-up issue
  `issues/reasoning_given_back_on_a_real_openrouter_call.md`. Finding 4 (echo every unknown reasoning-like field)
  not fixed: it would send fields the spec does not name, against TCL-18.
- Base commit: 2443ab9ae7f365f097b0a26050315e53c1c8241e
- OpenRouter probe: none (no key on the machine); its fields are followed from its docs and tested on a simulated
  OpenRouter-shaped answer; the exact shape of a real `reasoning_details` is not checked on a real call.

## Summary of deliverable 1
done: `Reasoning` keeps `reasoning_content`, `reasoning` and `reasoning_details` (raw JSON) and each goes back in its
own field only; spec back to `accepted`; `docs/crates/llm.md` updated.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass, TCL-20 traced |
| docs | `docs/crates/llm.md` |
| code review | low, no findings |

red: TCL-17, TCL-18, TCL-20: `unresolved import crate::Reasoning`, `no field reasoning_content on type String`

Code review: no findings

not verified: a real OpenRouter call (no key; decision 1 a); a model that degrades without its reasoning.
decisions needed: none

## Summary of deliverable 2
done: `Reasoning` (de)serialized by serde and flattened in both wire messages; `if_sent` crate-private; the fake
provider scripts a whole `Reasoning`; empty `reasoning` and `reasoning_details` checked sent back.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass |
| docs | n/a (no behaviour change) |
| code review | low, no findings |

red: TCL-19: `mismatched types: expected String, found Reasoning` (the fake provider scripted `reasoning_content` only)

Code review: no findings

not verified: a real OpenRouter call (follow-up issue).
decisions needed: none
