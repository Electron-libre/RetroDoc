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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
Every form that writes (review, edit, ask) shows pending (indicator + buttons disabled), saved (a fading notice) and
refused (the error inside the form, input kept); search and filters get the pending indicator. Out of scope: new
behaviour of the forms, the look as a requirement (checked by eye). Findings corrected: the review and edit forms
already swap their HTML error fragments (inline `before-swap` hooks, WEB-99); the ask form and plain-text errors show
nothing. Means: one page-wide rule in `base.html` (swap an HTML error, else a page notice) replaces the inline hooks;
the saved notice rides `?saved=<form>` on the redirects (works without JavaScript, and the edit's `HX-Redirect` reloads
the page) and the review fragment.

## Plan
1. [x] Pending: an indicator component (`.indicator`, text EN/FR) in the review, edit, ask, search and filter forms,
   `hx-indicator` + `hx-disabled-elt` on the three forms that write, htmx's injected indicator styles off; verified by
   markup tests of each page; makes true WEB-102. (autonomous)
2. [x] Errors: a `.form-error` slot above the buttons of each form (`role="alert"`), the review and edit failures
   rendered there; the ask refusals answered as the error fragment with `HX-Retarget`/`HX-Reswap`; one page-wide
   `before-swap`/`response-error` rule in `base.html` and a page notice region, the inline hooks removed; verified by
   tests of the answers (status, headers, fields kept) and of the base markup; makes true WEB-105, WEB-106, WAK-40. (autonomous)
3. [ ] Saved: `?saved=review|edit` on the `303` and `HX-Redirect`, the notice in the review fragment and on the pages
   (fades by CSS, the parameter dropped from the address bar once shown), texts EN/FR, `docs/dev/web-ui.md` states and
   screenshots; verified by tests of the redirects and of the pages with and without `?saved`; makes true WEB-83,
   WEB-97 (amended), WEB-103, WEB-104.

## Decisions
- Mode: autonomous
- Base commit: 891eb2879fd515a40886476886657e58f5d11c97
- Saved notice: `?saved=review|edit` on the redirects (the `303` and the `HX-Redirect`) plus the notice in the review
  fragment; the parameter dropped from the address bar once shown.
- Fixed error messages (unknown status, no such section, blank question, a question running, no docs) translated EN/FR
  in `texts.rs` (deliverable 2).

## Deliverable 1: pending
- done: `.indicator` (spinner + word, EN/FR) in the review, edit, ask, search, debt and rules forms; `hx-indicator`
  on each, `hx-disabled-elt` on the three that write; htmx's own indicator styles off.
- checks: tests ok, clippy ok, specs ok (WEB-102 traced), docs (`docs/dev/web-ui.md`, `docs/crates/render-mcp-cli.md`).
- red: WEB-102: `no hx-indicator="find .indicator"` in the opening tag of each form (assertion failed at form_states.rs).
- Code review: no findings
- not verified: the look in a browser (shot with deliverable 3).

## Deliverable 2: errors
- done: the error of a refused or failed review or edit in its form above the buttons (`.form-error`), the fields as
  sent; the ask refusals (400, 404, 409) answered to `htmx` as the error alone retargeted to `#ask-error`;
  `assets/forms.js` swaps an HTML error answer and shows any other in `#page-error`, the inline `before-swap` hooks
  removed; the fixed reasons EN/FR (`WebTexts::errors`).
- checks: tests ok, clippy ok, specs ok (WEB-105, WEB-106, WAK-40 traced), docs (`docs/dev/web-ui.md`,
  `docs/crates/render-mcp-cli.md`).
- red: WEB-105, WEB-106, WAK-40: `no <p class="form-error" role="alert">…</p>` in the form answered; no
  `/assets/forms.js` in the page; `hx-retarget` header missing on the ask refusals.
- Code review: no findings
- not verified: in a browser (driven with deliverable 3).
