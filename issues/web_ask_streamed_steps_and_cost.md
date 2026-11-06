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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
Show, while a question of the ask page runs, each tool call of the agent (tool and target), then the answer, then the
calls and tokens of the question and of the session. Out of scope: the answer streamed word by word, a cost in money,
stopping a question. Means: the question runs in a server task (no longer tied to the `POST`), followed by SSE (ADR 0049).

## Plan
1. [x] Agent observer: `Conversation::ask` and `ask_with` gain a variant that tells an observer each tool call (tool,
   target from `id`/`query`/`path`); verified by agent tests; makes true ASK-25. (autonomous)
2. [x] Streamed steps: the `POST` of `htmx` starts the question in a task and returns the running exchange; routes
   `/ask/questions/<n>/events` (SSE: steps, then the exchange) and `/ask/questions/<n>` (fallback); vendored
   `htmx-ext-sse` 2.2.4; button disabled until the answer; verified by `ask_page.rs` tests through the router (and a
   headless Chrome check of the page); makes true WAK-14, WAK-15, WAK-16 (amended), WAK-30 to WAK-35. (autonomous)
3. [x] Cost: a `UsageTracker` per question around the provider (`impl LlmProvider for Arc<P>`, `CallTotals` summable
   publicly), the session sum kept and reset with it, shown under each ended exchange in English and French; verified
   by `ask_page.rs` tests with a fake provider reporting usage; makes true WAK-36, WAK-37. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 16016b9e72251d8228814b3bc088c6a0d598cec5
- A question whose tab closes runs to its end, its exchange kept in its session (WAK-35, ADR 0049).

## Summaries

### 1. Agent observer
- done: `Conversation::ask_observed` tells an `Observer` each call as a `Step` (tool, target); `ask` is it with a no-op.
- checks: tests ok, clippy ok, specs ok (`specs/ask.md` back to `accepted`), docs `docs/crates/agent.md`, code review low.
- red: ASK-25: cannot find type `Step`, no method `ask_observed` on `Conversation`.
- Code review: no findings
- not verified: none. decisions needed: none.

### 2. Streamed steps
- done: a question runs in a task (`run`); the htmx `POST` returns the running exchange, which follows
  `/ask/questions/{n}/events` (SSE ext 2.2.4 vendored) and falls back to `/ask/questions/{n}`; the ended exchange lists
  its steps under a `<details>`; `serve` waits for the questions in flight at shutdown (`InFlight`), so the recap of
  WAK-23 still holds them; ADR 0049 accepted.
- checks: tests ok (`ask_page.rs`, `ui_ask.rs`), clippy ok, specs ok, docs (`render-mcp-cli.md`, `ARCHITECTURE.md`,
  README, CLAUDE.md), code review low.
- red: WAK-14, WAK-30, WAK-31, WAK-32, WAK-33, WAK-34: the htmx `POST` answered the whole exchange, no `sse-connect`
  (ParseIntError on the question number), no `/ask/questions/{n}` routes.
- Code review: 1 finding: `hx-disabled-elt` may re-enable the button after the `after-settle` handler disabled it;
  not changed: the headless Chrome check samples the button disabled throughout the question (htmx re-enables it
  before the settle).
- headless Chrome on a copy of the saved linkding run with a fake OpenAI server (1.5 s per call, 3 rounds): steps
  appear one by one, button disabled until the answer, `3 steps` summary; with `*/events` blocked, the fallback shows
  the answer.
- not verified: a real model (the DeepSeek hand check of the ledger, cents, needs a go). decisions needed: none.
- follow-up: `retrodoc-cli/tests/ui_ask.rs` WAK-23 now waits for the question in flight at the interrupt.

### 3. Cost
- done: each question asked through a `UsageProvider` of its own (`impl LlmProvider for Arc<P>`, `CallTotals::add`
  public); each ended exchange shows the calls and tokens of the question and of the session (`Session::spent`, reset
  with it), in English and French; `specs/web-ask.md` back to `accepted`.
- checks: tests ok, clippy ok, specs ok, docs (`render-mcp-cli.md`, `llm.md`, README), code review low.
- red: WAK-37: `Arc<Billed>: LlmProvider` is not satisfied (WAK-36 tests then failed on the missing cost line).
- Code review: no findings
- not verified: a real model (DeepSeek hand check, needs a go). decisions needed: none.
