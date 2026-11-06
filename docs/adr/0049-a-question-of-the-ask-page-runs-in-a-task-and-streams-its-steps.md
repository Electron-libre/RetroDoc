# 0049. A question of the ask page runs in a task and streams its steps

Status: Proposed

## Context

A question of the ask page (ADR 0048) takes tens of seconds and several rounds of tool calls: one `retrodoc ask` on
a saved run ran 8 rounds in 29 s (`issues/done/reasoning_given_back_on_a_real_openrouter_call.md`). The page answered
the `POST` of the question once the agent was done, behind a waiting indicator: the reader could not tell the
question was moving, nor what it cost. The interface renders its HTML on the server and drives it with `htmx`, its
scripts vendored (ADR 0039).

## Decision

This amends ADR 0048. The `POST` of a question sent by `htmx` starts the question in a task of the server and answers
at once with a running exchange; the agent tells an observer each tool call it runs (`specs/ask.md` ASK-25), which
the task keeps with the question. The running exchange follows the question with server-sent events, read by the SSE
extension of `htmx`, vendored like `htmx`: one event per step, replayed from the first on each connection, then the
rendered exchange. When the stream fails, the page asks for the exchange by a plain request that waits for the end of
the question. The question runs to its end whether a page follows it or not; its exchange is kept in its session. The
calls and tokens of a question are counted by a `UsageTracker` of its own around the provider of the pass `ask`, which
still counts them for the recap of `ui`.

Rejected: running the agent inside the stream request (a broken stream would lose the question, leaving the fallback
nothing to show); a WebSocket (two-way, needs a protocol and a second extension for a one-way stream); polling a
steps fragment (a request per second per tab, and a delay on each step).

## Consequences

A question is no longer dropped when its tab closes: it ends within its rounds, at its cost. The ask page gains a
script asset, the SSE extension, and the routes `/ask/questions/<n>` and `/ask/questions/<n>/events`. A proxy that
buffers the stream leaves the page on the fallback: the answer still shows, without the steps.
