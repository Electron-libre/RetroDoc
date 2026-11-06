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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
`retrodoc ui --ask` serves `/ask`: a conversation per browser session (cookie, in memory, 32 sessions at most), each
answer rendered from its Markdown with its sources linked to their pages and its warnings, the tokens counted under
the pass `ask` and saved when Ctrl-C stops `ui`. Out of scope: the steps and cost shown as it runs (next issue).

## Plan
1. [x] Ask page in `retrodoc-web` (ADR 0048, `specs/web-ask.md`): a router option holding the provider,
   `/ask` page and `POST /ask` (htmx fragment or 303), answer Markdown, sources as links, warnings, error exchange,
   blank question 400, waiting indicator, session cookie, English and French texts; verified by router
   tests with the fake provider (TCL-10); makes true WAK-2, WAK-6 to WAK-19. (autonomous)
2. [x] Sessions bounded, start over and the header link (WEB-101), `retrodoc ui --ask` in the CLI: 32 sessions least recently used dropped, 50
   exchanges kept, `POST /ask/reset`; the flag building the provider of the pass `ask`, failing at start with no
   docs or no key, the pass `ask` counted, graceful stop on Ctrl-C with the recap saved under `ui`; verified by binary
   tests with the fake LLM server (`tests/support`); makes true WAK-1, WAK-3 to WAK-5,
   WAK-20 to WAK-24. (autonomous)
3. [ ] Docs of the whole issue and fixes of the `high` review (`docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md`,
   README, CLAUDE.md diagram); verified by `project-check` and the review.

## Decisions
- Mode: autonomous
- Base commit: d8d941571b5429dd71d2b10d94d0ad74f7131cd9
- Session id: 128 bits of `getrandom` (already in the lock file, through `rand`).
- The usage is saved once, when `ui` stops on Ctrl-C (graceful shutdown), as `chat` does (CHT-10), not per question (answer 1 a).
- Deliverable 1 over the size limit: the bound of the sessions, the start over and the header link (WEB-101) stay in
  deliverable 2, with the `--ask` flag they go with.
- The link to `/ask` stands apart in the header, outside the groups of WEB-67 (answer 2 a).

## Deliverable 1
- done: `router_with(.., Some(Asking))` routes `/ask` (page and `POST`) in `retrodoc-web`, a `Conversation` per
  session cookie, the answer rendered from its Markdown, its sources linked, its warnings, English and French.
- red: WAK-2, WAK-6 to WAK-19: `unresolved import retrodoc_web::Asking`, `cannot find function router_with`.
- Docs (`CLAUDE.md` diagram, `docs/ARCHITECTURE.md`, `docs/crates/render-mcp-cli.md`) set aside for deliverable 3
  (size limit), in `~/retrodoc-runs/web_ask_d3_docs.patch`; the header link (WEB-101) for deliverable 2, in
  `~/retrodoc-runs/web_ask_d2_header_link.patch`.
- checks: project-check passed; deliverable-size 1000/1000; docs set aside as above.
- Code review: 3 findings
  - sessions unbounded: planned, WAK-20 and WAK-21 in deliverable 2;
  - misplaced doc comment of `french_refusal` in `texts.rs`: fixed;
  - a stale cookie loses the question on the `303`: not a defect, the new session's cookie is set on the `303`,
    which the browser keeps when it follows it.
- not verified: the page in a real browser (htmx indicator, swap); done with deliverable 2, once `ui --ask` exists.
- decisions needed: none.


## Deliverable 2
- done: `retrodoc ui --ask [--max-rounds N]` builds the provider of the pass `ask`, fails at start with no docs or no
  key, stops gracefully on Ctrl-C with the recap saved under `ui`; 32 sessions and 50 exchanges kept, `POST
  /ask/reset`, the header link (WEB-101); `specs/web-ask.md` and ADR 0048 accepted; the docs of deliverable 1 applied.
- red: WAK-3, WAK-4, WAK-5, WAK-23, WAK-24: `error: unexpected argument '--ask' found`.
- checks: project-check passed; deliverable-size 456; check-specs clean, spec accepted.
- Code review: 1 findings
  - `NO_DOCS` split the doc comment of `load_docs`: fixed.
- not verified: the page in a real browser (htmx indicator, swap, reset): asked at the end of the issue.
- decisions needed: none.
