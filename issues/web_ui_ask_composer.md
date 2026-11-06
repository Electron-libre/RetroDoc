---
priority: medium
depends_on: [web_ui_form_feedback]
why: 'the ask page is the most interactive one and reads as a bare text area'
---
# Make the question form of the ask page a composer

# Goal
The ask page holds a three-line text area and a "Send" button, and a "Start over" button that looks the same.
Expected: a composer as users expect of a chat: Enter sends, Shift+Enter breaks the line, the area grows with the
text, a few suggested questions when the conversation is empty, "start over" as a quiet action, the focus back in
the area after an answer.

# Findings
* `templates/ask.html`: `form.ask` (textarea `rows="3"`, `hx-post="/ask"`, `hx-swap="beforeend"`, reset on success)
  and a second form for `/ask/reset`.
* The steps of an answer are streamed (ADR 0049, `htmx-ext-sse`); the send button is disabled while a question runs.

# Approach
1. Keyboard: `hx-trigger="submit, keydown[key=='Enter'&&!shiftKey]"` or a few lines of script; growth with CSS
   (`field-sizing: content`) where supported, else a fallback.
2. Suggested questions: fixed ones, or built from the docs of the last run (domain names), as buttons that fill the
   area. Choose at the plan.
3. Focus and scroll to the new exchange after the answer.

# Specs
* `specs/web-ask.md`: adds the keyboard and the suggested questions requirements.

# Milestone
A web interface that invites to act

# Resources
* `templates/ask.html`, `exchange.html`, `step.html`; `crates/retrodoc-web/src/ask.rs`
* `specs/web-ask.md`, ADR 0048, ADR 0049; test `crates/retrodoc-web/tests/ask_page.rs`

# Hints
* A question costs a paid LLM call: the browser check of this issue uses a fake provider or a local model, ask the
  user before a paid one.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
Turn the ask form into a chat composer: Enter sends, Shift+Enter breaks the line, the field grows, suggested
questions on an empty conversation, a quiet "start over", focus and scroll back after an answer. Out of scope: any
LLM call to build the suggestions, a change of the routes or of the streaming. Means: a small `ask.js` beside
`forms.js`, CSS `field-sizing: content` with a script fallback, suggestions built from the docs of the last run.

## Plan
1. [x] Composer keyboard, growth, focus and quiet reset: `ask.js` (Enter/Shift+Enter, IME, growth fallback, focus and
   scroll after the `answer` swap), `rows="1"` + CSS, reset button quiet and shown only with an exchange or a running
   question; verified by `ask_page.rs` tests on the page markup and the served script; makes true WAK-41, WAK-42,
   WAK-45, WAK-46. (autonomous)
2. [x] Suggested questions: built in `ask.rs` from the purpose question and the first 3 domains of the last run,
   texts in English and French, buttons filling the field; verified by `ask_page.rs` tests (empty session shows
   them, a session with an exchange does not); makes true WAK-43, WAK-44; spec back to `accepted`. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 3e41799209242bec60906b9fa2e85ad212bd2d9b
- Suggestions: a fixed purpose question, then one per domain, the first 3 of the last run (no LLM call).
- Start over: shown only while the session holds an exchange or a question running.
- Growth: up to 12 lines, then the field scrolls.

## Deliverable 1
- done: `assets/ask.js` (Enter sends, Shift+Enter and IME keep the line, growth fallback, focus and scroll after an
  answer, reset shown once an exchange is there), one-line field growing to 12 lines in CSS, keys hint, reset form
  `hidden` on an empty conversation.
- checks: tests ok, clippy ok, specs ok, docs ok (`render-mcp-cli.md`, `web-ui.md`).
- Code review: no findings.
- red: WAK-41, WAK-42, WAK-45: the field was `rows="3"` and no `/assets/ask.js`; WAK-46: the reset form not `hidden`
  on an empty conversation.
- browser check (headless Chrome, `ui --ask` on a copy of `web_ask_streamed_check/repo`, a local fake LLM): Shift+Enter
  breaks the line, the field grows 39 → 60 → 270 px (capped), Enter sends and clears it, the focus is back in the field
  after the answer, the reset shows.
- not verified: an input method composition (IME) with a real keyboard; a browser without `field-sizing` (Firefox).

## Deliverable 2
- done: `ask::suggestions` (the purpose question, then the first 3 domains of the run, English and French),
  `ul.suggestions` above the form on an empty conversation, `ask.js` fills the field on a click and drops them once an
  exchange shows; spec `accepted`.
- checks: tests ok, clippy ok, specs ok, docs ok (`render-mcp-cli.md`, `web-ui.md`).
- red: WAK-43, WAK-44: the page held no suggested question (`left: []`).
- Code review: no findings.
- browser check (same setup): 4 suggestions shown, a click fills the field with the focus and sends nothing, Enter then
  sends it and the suggestions go.
- not verified: none beyond deliverable 1.

