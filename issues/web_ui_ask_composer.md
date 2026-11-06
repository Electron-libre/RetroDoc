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
