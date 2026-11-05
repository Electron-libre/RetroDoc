---
priority: medium
depends_on: [ask_command]
why: 'the loop works on the fakes; a local model or a strict server can still lose the answer after the paid rounds'
---
# Keep the loop of `retrodoc ask` within the context and the rules of every server

# Goal
The agent of `retrodoc ask` bounds its rounds of tool calls (`--max-rounds`), but neither the calls of one round nor
the size of the tool results, and its last request drops the tools while the history still holds tool calls. Either
can make the question fail with no answer after the rounds were paid. Expected: the tool results of a question stay
within a budget, and the last request is accepted by the servers that refuse a tool history without tools, so the
model always gets to answer.

# Findings
* 2026-11-05, `high` review of `issues/done/ask_command.md`, read from the code, not seen on a real run: one round may hold
  any number of calls; a `read_source` result reaches 400 lines of up to 400 characters, so a round of many reads
  can push the messages past a local model's context, and the next request fails (`crates/retrodoc-agent/src/lib.rs`,
  `ask`).
* Same review: the last request (after `max_rounds`) sends no `tools` while the history holds assistant tool calls
  and tool results; some backends reached through `OpenRouter` are said to refuse a tool history without tool
  definitions (a 400). Not checked on a real call.

# Approach
1. A budget of the tool results of a question (calls per round, characters in all): a call over it answered with an
   error that tells the model to narrow down or answer; the figures chosen from the context of the local model
   (`docs/dev/commands.md`, local LLM setup).
2. The last request: keep offering the tools with a `tool_choice` of `none` (a new field of `CompletionRequest`,
   `specs/tool-calling.md` lists it as a non-goal today), or fold the history into plain text; check one real call on
   `OpenRouter` first (cents, ask before).
3. Tested with the scripted fake provider, like `ask`.

# Specs
* `specs/ask.md`: new requirements (budget of the tool results, the last request); `specs/tool-calling.md` if
  `tool_choice` is added.

# Milestone
Ask the documentation

# Resources
* `specs/ask.md` (ASK-6), ADR 0046, ADR 0047, `docs/crates/agent.md`, `crates/retrodoc-agent/src/lib.rs`,
  `issues/done/tool_loop_on_thinking_and_refusing_servers.md` (the other server cases of the loop).

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
Bound what the tool results of one question can add to the context (calls per round, characters in all) and make
the last request free of any tool history, so every server accepts it and the model always answers. Out of scope: a
`tool_choice` field (not needed once the history is folded), a budget in tokens. Means: fold the history into text
rather than `tool_choice: none`, which needs no paid check and no server support.

## Plan
1. [x] Budget and folded last request: `ask_with` answers the calls past 6 in a round and the results past 60000
   characters with an error, and its last request folds each call and result into one user message; verified by
   tests with the scripted `FakeLlm` in `crates/retrodoc-agent/src/tests.rs` and the end-to-end `ask` test; makes
   true ASK-22, ASK-23, ASK-24. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 59fd8e2ced9f7139a0eadc99f5abf4a2d3a96291
- Last request: the history folded into text, not `tool_choice: none` (1 a).
- Budget: 6 calls per round, 60000 characters per question (2 a).

## Deliverable 1
- done: `ask_with` answers the calls past 6 in a round and a result past 60000 characters with an error, and its
  last request folds the calls and results into one user message (no tool history).
- red: ASK-22, ASK-23, ASK-24: roles `[System, User, Assistant, Tool, User]` instead of `[System, User, User]`; the
  7th call ran `list_domains`; the result over the budget was returned whole; end to end, the last request sent the
  tool result.
- Code review: no findings.
- not verified: a real call on a local model or `OpenRouter` (fakes only).
