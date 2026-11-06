---
priority: high
depends_on: [web_ui_components]
why: 'the forms that write (review, edit, ask) give no sign of what they are doing, the core of the milestone'
---
# Show the state of each form while it is sent, saved or refused

# Goal
The review, edit and ask forms are sent with `htmx` but show nothing while the request runs, nor that it succeeded,
and an error replaces the section with a message. Expected: each form shows its state: pending (an indicator, the
submit button disabled), saved (a short notice that fades), refused (the error beside the field, the input kept),
the same way on every form.

# Findings
* `templates/review.html` and `templates/edit_form.html` swap `closest section` on answer; no `hx-indicator`, no
  `hx-disabled-elt`.
* `templates/ask.html` disables its button during the request (`hx-disabled-elt="find button"`) and resets the form
  on success; it is the only one.
* `review_form.rs` answers an unknown status with 400 and a failed save with 500; `edit_form.rs` answers 400 and 500
  the same way; `htmx` swaps nothing on a 4xx/5xx by default, so the user sees no error unless the page handles it
  (WEB-86 to WEB-88).

# Approach
1. A shared pattern: `hx-indicator` and `hx-disabled-elt` on every form, an indicator component from
   `web_ui_components`.
2. Saved notice: the server answers with an `HX-Trigger` header (for example `{"notice": "..."}`) and a small region
   of `base.html` shows it; or the swapped fragment carries the notice. Choose at the plan.
3. Errors: let `htmx` swap the 4xx answers (`htmx.config.responseHandling` or `hx-target-error` of an extension)
   so the error appears beside the field with the input kept; keep the status codes of the spec.

# Specs
* `specs/web-ui.md`: amends WEB-83, WEB-86 to WEB-88 (review answers) and the answers of the edit form (WEB-93 and
  after); adds the pending and saved states.
* `specs/web-ask.md`: the pending state of the ask form if it changes.

# Milestone
A web interface that invites to act

# Resources
* `crates/retrodoc-web/src/review_form.rs`, `edit_form.rs`, `ask.rs`; `templates/review.html`, `edit_form.html`,
  `ask.html`, `base.html`
* `crates/retrodoc-web/tests/review_pages.rs`, `edit_pages.rs`, `ask_page.rs`
* htmx docs on `responseHandling`, `hx-indicator`, `HX-Trigger`

# Hints
* Every new text exists in English and French (`texts.rs`, WEB-7x on the language of the interface).
* The forms also work without JavaScript (plain `POST` and `303`): keep that path.
