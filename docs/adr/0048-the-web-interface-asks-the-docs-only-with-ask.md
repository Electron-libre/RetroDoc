# 0048. The web interface asks the docs only with `--ask`

Status: Proposed

## Context

`retrodoc ask` and `retrodoc chat` question the generated docs through the agent of `retrodoc-agent` (ADR 0047), in
the terminal only, while the reader browses the docs in the web interface, where the pages of the sources of an
answer live. The interface calls no LLM (ADR 0039): it works offline, costs nothing and sends no line of a
confidential repository to a third party. A question costs: one `ask` on a real run took 9 calls, 27,161 tokens in
and 29 s (`issues/done/reasoning_given_back_on_a_real_openrouter_call.md`). A reader asks follow-ups, which `chat`
answers with the earlier turns kept in memory (`specs/chat.md`).

## Decision

This amends ADR 0039. `retrodoc ui` calls the LLM only when it is started with `--ask`; it then serves an ask page
holding a conversation like `chat`: one `Conversation` of `retrodoc-agent` per browser session, its id in a cookie,
kept in memory and bounded in number, with the model and the tokens of the pass `ask`. Without `--ask` it builds no
provider, offers no ask page and stays LLM-free. `retrodoc-web` depends on `retrodoc-agent` and `retrodoc-llm`; the
CLI builds the provider and gives it to the router.

Rejected: the page on whenever an LLM is configured (a configured key is no consent to send the docs from a browser
tab, and the cost would follow a click); one question per page with no memory (a follow-up would restate its context,
unlike `chat`).

## Consequences

The architecture diagram gains `retrodoc-web ──> retrodoc-agent, retrodoc-llm`. The sessions are lost at a restart,
and the server, with no authentication, keeps binding to `127.0.0.1` by default: anyone who reaches it can spend the
key. `ui` stops gracefully on Ctrl-C so its recap is printed and saved. Showing the steps of the agent and the cost
as a question runs is a follow-up (`issues/web_ask_streamed_steps_and_cost.md`).
