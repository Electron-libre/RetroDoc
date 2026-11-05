---
priority: medium
depends_on: [edit_store, edits_in_read_model]
why: 'the milestone''s entry point: without the form nobody can edit a section'
---
# Edit a section from its page in the web interface

# Goal
The page of a domain, a feature or a use case shows the docs and takes a review, not a correction. Expected: a form
on the page edits its name, description and narrative, saved through the store of `edit_store`; the page says the
section was edited by a human (author, date), shows the generated text it replaces, and brings it back on request.

# Findings
* 2026-11-05: the review form (`crates/retrodoc-web/src/review_form.rs`) already posts to the URL of the page with
  `htmx`, refuses a cross-site `POST` (`refuse_cross_site`, WEB-84) and answers a fragment to `htmx`, a redirect
  otherwise.
* 2026-11-05: `specs/web-ui.md` lists "no edit of the docs" as a non-goal: this issue amends it.
* 2026-11-05: `edits::record` refuses a blank field (SED-5) but saves the others as given: a textarea sends
  surrounding whitespace and newlines; decide whether the form or the store trims them (high review of
  `edit_store`).

# Approach
1. An edit form beside the review form, the fields filled with the text shown; a `POST` saves the edit, an empty
   field or "back to the generated text" removes its part of the edit.
2. The mark "edited by a human" in the lists of sections, and the generated text folded under the edit on the page;
   the edit on the page (fields, author, date, outdated) is shown by `edit_outdated` (WEB-91, WEB-92).
3. The same checks as the review form: same origin, a section of the run, a store that can't be read.

# Specs
* `specs/web-ui.md`: the edit form, the mark of an edited section; amend the non-goal.

# Milestone
Edit the docs

# Resources
* `crates/retrodoc-web/src/review_form.rs`, `review.rs`, `templates/review.html`, `specs/web-ui.md` WEB-79 to
  WEB-90.

# Hints
* `htmx` keeps the swap of an error answer only through the `before-swap` hook of the review section: a fragment
  must start with its `<section`, whitespace included.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
A form on the page of a domain, feature or use case writes its edit (name, description, narrative) through
`edits::record`/`edits::remove`; the lists mark an edited section and the page folds the generated text under the
edit. Out of scope: steps, rules, glossary; per-field revert. The form trims and normalises line ends (WEB-94), not
the store; a field equal to the generated text is left out (WEB-95); success reloads the page (`HX-Redirect`).

## Plan
1. [x] Edit form and its `POST` (`edit_form.rs`, `/edit/domains/…` routes, `edit.html` form, texts en/fr); verified by
   `tests/edit_pages.rs` through the router; makes true WEB-93 to WEB-99, WEB-12 (amended). ~650 lines. (autonomous)
2. [ ] Mark of an edited section in the lists and the generated text folded under the edit; verified by
   `tests/edit_pages.rs`; makes true WEB-100, WEB-92 (amended). ~250 lines.

## Decisions
- Mode: autonomous
- Base commit: 21c137465c5c4ded2a4a0d2afa7d80162b84e611
- 2026-11-05 (user, `ok`): the form trims and turns `\r\n` into `\n` (not the store); a field equal to the
  generated text is left out; a saved edit answers `HX-Redirect` (whole page reload); one button removes the whole
  edit, emptying a field removes that field.
- 2026-11-05 (agent): the form posts to `/edit<page URL>`, not `<page URL>/edit`, which would shadow a feature
  or a use case whose slug is `edit` (axum prefers a static segment).

## Deliverable 1 summary
done: edit form folded under the review of each section page, `POST /edit<page>` saving through `edits::record`/`remove`.

| check | result |
|---|---|
| tests | pass (`project-check`) |
| lint | pass |
| specs | pass (`web-ui` back to `draft` until WEB-100 is traced) |
| docs | README, `ARCHITECTURE.md`, `docs/crates/render-mcp-cli.md` |
| code review | 1 finding, fixed |

red: WEB-93, WEB-94, WEB-95, WEB-96, WEB-97, WEB-98, WEB-99: `no edit form in <!doctype html>` and `left: 404` (no route).
Code review: 1 findings: a narrative refused for a domain or a feature answered the form without its revert button on
an edited section (`edited` forced to `false`); fixed, the refusal moved into `write`, test added.
not verified: the form in a real browser (htmx `HX-Redirect`, the submitter `action=revert`): checked in deliverable 2.
decisions needed: none.
