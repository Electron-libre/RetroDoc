---
id: FIX
status: accepted
adrs: [0039, 0044, 0045]
---

# Fixed items of the debt

## Intent

The debt page (`specs/debt-view.md`) lists apart, under "Fixed", the sections a human reviewed, validated or marked
wrong, so that whoever works through the debt sees the items left shrink to the work left, and sees what they
settled.

## Requirements

### Behaviour

- **FIX-1** (deterministic) The debt page must show, under a heading "Fixed" after the items left, each domain,
  feature and use case of the run that a human settled: its review is `validated` or `wrong` and not outdated
  (`specs/section-review.md` REV-20, REV-21), or its edit, saved at or after that review, is not outdated
  (`specs/section-edit.md` SED-34, SED-35).
- **FIX-2** (deterministic) A fixed section must not be listed among the items left under the kind "marked wrong"
  (`specs/debt-view.md` DBT-30) nor under the kinds of judgement (low confidence, not scored, unbacked narrative,
  technical language, DBT-32).
- **FIX-3** (deterministic) Each fixed section must show the verdict of its review (validated, or wrong with its
  comment), the author and the date of the review, the author and the date of the edit that keeps it settled when
  there is one (FIX-1), and a link to its page.
- **FIX-4** (deterministic) Each fixed section must name the kinds of the items it took out of the items left: the
  kinds of judgement it falls in, and "marked wrong" for a section marked wrong; a validated section that falls in no
  kind of judgement names none.
- **FIX-5** (deterministic) A fixed section whose review becomes outdated, with no later edit up to date, must leave
  "Fixed" and be listed again among the items left under the kinds it falls in, a section marked wrong as "marked
  wrong" said outdated with its reasons (DBT-37).

### Contract

- **FIX-6** (deterministic) The fixed sections must be left out of the number of items of the debt (the link of the
  home page, DBT-13) and of the priority order (DBT-23).
- **FIX-7** (deterministic) The fixed sections must be ordered by the date of their review, or of their edit when
  later, the newest first, then by kind (domain, feature, use case) and id.
- **FIX-8** (deterministic) The filter `?domain=<slug>` must keep the fixed sections of that domain, and `?kind=<kind>`
  the fixed sections that took out an item of that kind (FIX-4).
- **FIX-9** (deterministic) The heading "Fixed" must not be shown when no fixed section is kept by the filters.
- **FIX-10** (deterministic) The answer to a debt request with `HX-Request` (DBT-12) must hold the fixed sections
  with the items left.

## Scenarios

- **FIX-H1** (happy; FIX-1, FIX-2, FIX-3, FIX-4, FIX-6) Given a use case of low confidence, when the reviewer marks
  it wrong with the comment "wrong actor", then the debt page lists it neither as low confidence nor as marked wrong,
  the number of items drops by one, and "Fixed" shows it wrong, "wrong actor", by the reviewer at the date of the
  review, having taken out "low confidence" and "marked wrong".
- **FIX-H2** (happy; FIX-1, FIX-3, FIX-4) Given a feature marked wrong, when the reviewer corrects its description in
  the edit form, then it stays under "Fixed", now also showing the edit, its author and its date.
- **FIX-H3** (happy; FIX-1, FIX-3, FIX-4) Given a use case of low confidence, when the reviewer validates it, then
  "Fixed" shows it validated, by the reviewer, having taken out "low confidence".
- (FIX-1) Given a section left `to_review`, then it is not fixed, edited or not.
- (FIX-1) Given a section edited, when it is reviewed next, then it is fixed by its review alone: the review holds
  the fingerprint of the section edited (SED-26).
- (FIX-1) Given a section reviewed and edited in the same second (one form saving both), then it is fixed.
- (FIX-1, FIX-4) Given a domain validated, then it is fixed with no kind taken out: a domain falls in no kind of
  judgement.
- (FIX-5) Given a feature marked wrong then corrected by an edit, when a later run rewrites it, then it is listed
  again as marked wrong, said outdated, and as an outdated edit, "rewritten" (DBT-39).
- (FIX-5) Given a use case validated, when a file one of its steps cites is edited, then it leaves "Fixed" and is
  listed again under the kinds of judgement it falls in.
- (FIX-1) Given a review of a section the run no longer holds, then nothing is fixed for it.
- (FIX-1, FIX-7) Given a review written by hand at `10:00:00+02:00` and an edit at `09:00:00Z`, then the edit is the
  later one: the dates compare as instants; a date that does not parse keeps no section settled by its edit and
  orders last.
- (FIX-8) Given a filter `?kind=stale`, then no fixed section is kept: a review takes out no stale item.

## Non-goals

- No new status of the review and no change of `.retrodoc/reviews.json` nor `.retrodoc/edits.json`: "fixed" is read
  from the reviews and the edits as they are saved, never recorded.
- A review does not take out the kinds stale, contradicted, set apart, undocumented, outdated edit nor the code no
  doc covers: it vouches for the section, not for the project's docs nor the files changed after it.
- No judgement of whether an edit corrects what the review said wrong: an edit saved after a review keeps the
  section settled; a reviewer who disagrees reviews it again.
- `retrodoc report` keeps listing the sections marked wrong (DBT-35); it, `coverage-report.md` and the run history
  list no fixed section; the audit dashboard keeps listing the reviews as saved.
- The run history counts the debt as the page lists it, the settled sections left out; no snapshot saved before is
  counted again, so the trends of the audit page show a drop at the first run after a section is settled.
- No history of the resolutions: a section reviewed twice shows its last review.
