---
priority: medium
why: 'every correction of the reader goes through these forms; two forms side by side slow the reading down'
---
# Review and edit a section in one interaction

# Goal
The page of a domain, a feature or a use case holds two forms, one to review the section and one to edit it, and
the user finds the pair impractical. Expected: one interaction to judge a section and correct it, so that the usual
paths (validate, mark wrong and fix, leave a note to review) take the fewest steps.

# Findings
* The page holds the review form (`specs/web-ui.md` WEB-81) and the edit form (WEB-93), each with its own submit,
  notice and error (WEB-102 to WEB-105); the edit form is folded unless asked with `?edit=1` (WEB-114).
* The debt page offers two actions per item, "Review" and "Edit" (WEB-115).
* An edit saved after a review makes that review outdated (`specs/section-edit.md` SED-26): correcting a section
  marked wrong takes two saves and leaves the review said outdated.

# Approach
1. List with the user the paths a reviewer takes and the steps each takes now (screenshots of
   `just screenshots`).
2. Candidates (can be combined):
   1. one form: the status choices first, the editable fields unfolded on "wrong", one save writing the review and
      the edit together;
   2. editing in place on the page, the verdict buttons beside the heading;
   3. one action per debt item in place of "Review" and "Edit".
3. Decide what one save writes when both change, so the review is not outdated by its own edit.

# Specs
* `specs/web-ui.md` (WEB-81 to WEB-105, WEB-113 to WEB-121 amended), `specs/section-edit.md` SED-26.

# Milestone
Read by someone taking an application over

# Resources
* `docs/dev/web-ui.md`; `crates/retrodoc-web/src/review_form.rs`, `edit_form.rs`, `review.rs`, `edit.rs`.
* Closed issues `web_review_form`, `web_edit_form`, `web_ui_form_feedback`, `web_ui_edit_live_preview`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-24

## Understanding
One form per section page (`specs/section-form.md`, split out of `specs/web-ui.md`): the status and comment first,
the text fields folded and unfolded on "wrong", one save writing the edit then the review, the review fingerprinted on
the edited section so it is not outdated by its own edit; one "Review" action per page and per debt item. Out of
scope: in-place editing on the page (candidate 2), the steps, rules and glossary.

## Paths

| path | steps now | steps after |
|---|---|---|
| validate | Review, pick the status, Save (3) | Review, Validated, Save (3) |
| mark wrong and fix | Review, Wrong, comment, Save, Edit (reload), fields, Save: review outdated (7) | Review, Wrong (fields unfold), comment, fields, Save: review up to date (5) |
| leave to review | Review, To review, comment, Save (4) | Review, To review, comment, Save (4) |
| fix without a verdict | Edit (reload), fields, Save (3) | Review, unfold the fields, fields, Save (4) |

## Plan
1. [x] Store of one save: `retrodoc_docs::corrections::save` (new module) checks both files readable, writes or
   removes the edit, then records or removes the review on the docs with that edit applied; the spec split
   committed with it; verified by internal tests of the docs crate; makes true SFM-4, SFM-14 (store side). (autonomous)
2. [ ] One form, one `POST`: `section_form.rs` (new, replacing the saves of `review_form.rs` and `edit_form.rs`, the
   preview kept) and `section_form.html` (replacing the form of `review.html` and `edit_form.html`), the routes under
   `/edit` removed; new tests in `tests/section_form.rs`, the obsolete `POST` tests of `review_pages.rs`,
   `edit_pages.rs` and `form_states.rs` (both over 800 lines) removed or moved to the new path; verified by tests
   through the router; makes true SFM-1, SFM-3, SFM-4, SFM-5 (save), SFM-9 to SFM-15.
3. [ ] The page around it: the radio chips of the status, the fields unfolded on "wrong" (`forms.js`) and on an
   error, one "Review" action under the heading and per debt item, the notice of `?saved=1`, the indicator, the
   confirmation of the revert; `texts.rs` (over the limit) only swaps its texts of the two forms, no new block;
   verified by page tests and before/after `just screenshots`; makes true SFM-2, SFM-5 (confirm), SFM-6, SFM-7,
   SFM-8, SFM-16.
4. [ ] Docs of the whole issue and fixes of the `high` review (`docs/dev/web-ui.md`, `docs/crates/render-mcp-cli.md`,
   `docs/crates/docs.md`); spec set `accepted`.

## Decisions
- Mode: autonomous
- Base commit: 0b8fb86d1de6ceade2ba28a1107b6a8c3ffd1b57
- An `htmx` save always reloads the page (`HX-Redirect` to `?saved=1`), no fragment answer (user, 2026-11-24).
- A save rewrites the review as the form sends it, the status kept included: its author becomes whoever saved, and it
  is fingerprinted on the edited section (user, 2026-11-24).
- The status is a group of radio chips, not a select (user, 2026-11-24).
- Spec: the section forms moved out of `specs/web-ui.md` into `specs/section-form.md` (SFM), the former ids
  withdrawn (user, 2026-11-24).

## Deliverable 1
- done: `retrodoc_docs::corrections::save` writes the edit (kept, set or removed) then the review fingerprinted on the
  sections with that edit applied (`reviews::record_in`); both files read before either is written; the spec split.
- | check | result |
  |---|---|
  | tests | `cargo test -p retrodoc-docs`: pass (7 new) |
  | lint | clippy pedantic: clean |
  | specs | `check-specs`: pass, `section-form.md` draft |
  | docs | `docs/crates/docs.md` |
  | code review | low: no findings |
- red: SFM-4: `a_review_saved_with_an_edit_is_not_outdated_by_it` failed, the review outdated "rewritten" when
  fingerprinted on the docs as shown before the edit.
- Code review: no findings
- not verified: nothing through the router yet (deliverable 2).
- decisions needed: none.
