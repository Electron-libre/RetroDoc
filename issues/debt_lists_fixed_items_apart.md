---
priority: medium
why: 'a reviewer cannot see their progress while the fixed items stay in the list to review; a comfort of the reading, not a blocker'
---
# List the fixed items of the debt apart from those to review

# Goal
When a user works through the debt, an item they settled keeps showing among the items to review. Expected: an item
the user resolved leaves the list to review and shows in a section "Fixed" of the debt page, so the list left is the
work left.

# Findings
* A section marked wrong then corrected with the edit form stays listed as "marked wrong", only said outdated: the
  edit changes the fingerprint the review holds (`specs/section-edit.md` SED-26, `specs/debt-view.md` DBT-30, DBT-37).
* A section marked `validated` leaves only the kinds low confidence, not scored, unbacked narrative and technical
  language (DBT-32); it stays listed under the other kinds (stale, contradicted, undocumented...).
* An edit that is not outdated is no debt (non-goal of DBT); there is no notion of a fixed item.

# Approach
1. Define "resolved" with the user, per kind of item: validated after the last change, edited after a `wrong`
   review, a `to_review` review changed to `validated`, and whether a later run or a file change brings it back.
2. Candidates (can be combined):
   1. a status `fixed` of the review, set by the edit of a section marked wrong or chosen in the form;
   2. a section "Fixed" of the debt page that lists the resolved items, with the date and author of the resolution,
      left out of the count and the priority of the debt;
   3. an item fixed then rewritten or whose files change goes back to the list, said why (REV-20, REV-21).
3. Decide whether `retrodoc report` and `coverage-report.md` say the fixed items too.

# Specs
* `specs/debt-view.md` (DBT-30 to DBT-39 amended, new requirements for the section "Fixed"),
  `specs/section-review.md` (REV-2 if a status is added), `specs/web-ui.md`.

# Milestone
Read by someone taking an application over

# Resources
* `crates/retrodoc-docs/src/debt/`, `crates/retrodoc-web/src/debt.rs`, `crates/retrodoc-web/src/review.rs`.
* `docs/dev/web-ui.md` (screenshots before and after).

# Hints
* The review form is being rethought (`one_form_to_review_and_edit_a_section`): agree on what resolving means
  there first, or do both together.
* A new review status changes the format of `reviews.json`: its version and the readers (ADR 0050).
