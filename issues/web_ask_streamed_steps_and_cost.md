---
priority: medium
depends_on: [web_ask_page]
why: 'a comfort over the waiting indicator: an answer takes tens of seconds'
---
# Stream the steps of the agent and the cost to the ask page

# Goal
An answer of the agent takes tens of seconds and several rounds of tool calls; with only a waiting indicator the
reader cannot tell it is moving nor what it costs. Expected: the ask page shows each tool call of the loop as it runs
(the tool and its target, such as `get_feature bookmark-sharing`), then the answer, and the calls and tokens of the
question and of the session.

# Findings
* 2026-11-06: one `retrodoc ask` on the saved linkding run ran 8 rounds of tool calls in 29 s
  (`issues/done/reasoning_given_back_on_a_real_openrouter_call.md`).

# Approach
1. A way for the caller of `retrodoc_agent` to see each call of the loop as it runs (a callback or a channel of
   events), the loop otherwise unchanged.
2. A server-sent events endpoint per question in `retrodoc-web`, read by the htmx SSE extension (vendored, as the
   other assets, ADR 0039); the page falls back to the whole answer when the stream breaks.
3. The calls and tokens of the question and the sum of the session under the answer, from the usage tracker.

# Specs
* `specs/web-ask.md` (written by `web_ask_page`): requirements for the streamed steps and the cost; `specs/ask.md`
  if the agent gains the events of 1.

# Milestone
Ask the documentation in the web interface

# Resources
* ADR 0039 (vendored assets), ADR 0048 (written by `web_ask_page`), `specs/web-ask.md`, `crates/retrodoc-agent/src/lib.rs`, `crates/retrodoc-web/src/assets.rs`,
  `docs/crates/agent.md`.

# Hints
* `retrodoc chat` prints a recap of its calls at the end: reuse its counting, don't add a second one.
