---
priority: high
why: 'the milestone is this page; the streaming builds on it'
---
# Ask the documentation from the web interface

# Goal
`retrodoc ask` and `retrodoc chat` question the docs from the terminal only, while the reader browses them in the web
interface, where the pages of the sources of an answer live. Expected: `retrodoc ui --ask` serves an ask page holding a
conversation like `retrodoc chat`, each answer with its sources as links to their pages and to the cited code, and its
warnings; without `--ask` the interface calls no LLM and offers no ask page.

# Findings
* 2026-11-06: one `retrodoc ask` on the saved linkding run took 9 calls, 27,161 tokens in, 1,388 out, 29 s
  (`issues/done/reasoning_given_back_on_a_real_openrouter_call.md`): the page must show that a question is running.
* `retrodoc_agent::Conversation::ask` keeps the earlier turns and answers with an `Answer` (text, sources, warnings);
  the sources are typed (`Source`), so links can be built from them, not parsed from the text.

# Approach
1. `--ask` on `retrodoc ui`: build the provider as `ask` does (`[llm]` with the pass `ask`), fail at start with the
   message of `ask` when there are no generated docs or no key.
2. The page: a form posting the question (htmx), the answer rendered from the Markdown of its text, the sources as
   links to the domain, feature, use case and code pages, the warnings; a waiting indicator while it runs.
3. One `Conversation` per browser session (a cookie id, in memory, bounded in number), a button to start over.
4. The tokens counted under the pass `ask` in the usage history, as `ask` does.

# Specs
* New `specs/web-ask.md` (the opt-in, the page, the session, the links of the sources); `specs/web-ui.md` amended for
  the link to the page when `--ask` is given.

# Milestone
Ask the documentation in the web interface

# Resources
* ADR 0039 (the interface calls no LLM), ADR 0047, `specs/ask.md`, `specs/chat.md`, `specs/web-ui.md`.
* `crates/retrodoc-agent/src/lib.rs` (`Conversation`, `Answer`, `Source`), the `chat` command of `retrodoc-cli`,
  `crates/retrodoc-web/src/lib.rs` and its review and edit forms (the htmx forms already there).
* `docs/crates/agent.md`, `docs/crates/render-mcp-cli.md`.

# Hints
* The page amends ADR 0039: write ADR 0048 at the start (agreed with the user on 2026-11-06: off by default, on with
  `--ask` only, a conversation like `chat` kept in memory per browser session, the model and tokens of the pass
  `ask`; rejected: on whenever an LLM is configured, one question per page with no memory).
* Test with the fake provider of the tests (TCL-10), through the HTTP router as the other web tests do; no paid call.
* The interface binds to `127.0.0.1` by default: keep it, the page sends the docs to the LLM.
