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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-24

## Understanding
A section a human reviewed, validated or marked wrong, leaves the items left of `/debt` and shows under "Fixed",
while its review is up to date or followed by an edit up to date; it comes back when outdated. Read from the saved
reviews and edits, no new status, no format change (`specs/debt-fixed.md`, FIX). `retrodoc report` unchanged.

## Plan
1. [x] Fixed sections in the debt model: `DebtList::fixed` in `retrodoc-docs` (new module `debt/fixed.rs`, its
   tests in a file of their own: `debt/mod.rs` is near the file limit), a fixed section left out of "marked wrong" and
   of the kinds of judgement; the DBT spec takes the Behaviour / Contract shape; verified by internal tests of the
   debt; makes true FIX-1, FIX-2, FIX-4, FIX-5, FIX-6, FIX-7. (autonomous)
2. [ ] Section "Fixed" of the debt page: rows with verdict, comment, author, dates, link and kinds taken out, the
   filters, the `HX-Request` answer, the texts, screenshots before and after; verified by web tests in a new
   `crates/retrodoc-web/tests/debt_fixed.rs` (`debt_page.rs` is over the file limit); makes true FIX-3, FIX-8,
   FIX-9, FIX-10, FIX-H1, FIX-H2, FIX-H3, DBT-H1.
3. [ ] Docs of the whole issue and the fixes of the `high` review.

## Decisions
- Mode: autonomous
- Base commit: 00c82b245fc0b396578e9990a33ccc5ade403587
- Fixed = a review `validated` or `wrong` up to date, or followed by an edit up to date (user, 2026-11-24: "si une
  section est marquée fausse alors elle est en effet fixed"); a wrong review outdated comes back as "marked wrong".
- No status `fixed`: the verdict stays the user's choice in the review form.
- `retrodoc report` and `coverage-report.md` unchanged: "Sections marked wrong" (DBT-35) keeps listing them.
- One spec of its own, `specs/debt-fixed.md` (DBT at 39 requirements, bound 40).
- Independent of `one_form_to_review_and_edit_a_section`: a review and an edit saved in the same second stay fixed.

## Deliverable 1: fixed sections in the debt model
- done: `DebtList::fixed` (`debt/fixed.rs`), a settled section takes its kinds of judgement and its marked wrong item
  out of the items; the DBT spec in the Behaviour / Contract shape, DBT-30 and DBT-32 amended.
- checks: project-check ok; specs ok (DBT-13 wording warning untouched); docs `docs/crates/docs.md`; code review
  below.
- red: FIX-1, FIX-2, FIX-4, FIX-6, FIX-7: `assertion failed: kinds_of(list, "billing/pay/refund") == [SetApart]`
  (left `[LowConfidence, SetApart, MarkedWrong]`), the fixed list empty.
- Three web tests of DBT-30, DBT-34 and AUD-6 adapted: an up-to-date wrong review is no longer debt, so they rewrite
  the run (`rewrite_run`) or use `to_review`.
- Code review: no findings.
- not verified: the page (deliverable 2).
