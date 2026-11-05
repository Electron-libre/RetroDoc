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
  `issues/tool_loop_on_thinking_and_refusing_servers.md` (the other server cases of the loop).
