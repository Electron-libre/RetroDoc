---
id: HCR
status: draft
adrs: [0037, 0044, 0045, 0056]
---

# Human corrections guide the next run

## Intent

The edits and the reviews a team made of the generated docs are read by the next `generate`: the passes that write
a domain, a feature or a use case are given the corrections of the sections they write, so that a run does not lose
or contradict what a human fixed, and a section marked wrong is written again.

## Requirements

### Behaviour

- **HCR-1** (deterministic) The request of the unit that writes an edited domain, feature or use case must hold the
  fields of its edit (name, description, narrative), given as the text to keep unless the code contradicts it.
- **HCR-2** (deterministic) The request of the unit that writes a section reviewed `wrong` must hold the comment of
  its review, given as what to fix.
- **HCR-3** (deterministic) The request of the unit that writes a section reviewed `validated` must hold that
  section, given as the text to keep.
- **HCR-4** (deterministic) A `generate` must ask again the unit that writes a section reviewed `wrong`, once per
  review: the same review, unchanged, asks it no more at the runs that follow.
- **HCR-5** (deterministic) An edit, or a review `validated`, must not by itself ask a unit again: it reaches the
  request the next time its unit is asked for another reason (a changed input, another model, a review `wrong`).

### Contract

- **HCR-6** (deterministic) The unit that writes a section must be: the domains call for a domain; the features call
  of its domain (the slice that holds it, for a domain asked in slices) for a feature; the use cases call of its
  feature for a use case; the domain of a feature and the feature of a use case taken from the previous run's saved
  results.
- **HCR-7** (deterministic) A request must hold the corrections of the sections its unit writes and no other.
- **HCR-8** (deterministic) A request must name each corrected section by its slug, as the answer gives it, so that
  the edit and the review still name the section the model writes again.
- **HCR-9** (deterministic) A review `to_review`, a blank field of an edit (SED-33) and a correction of a section no
  saved result holds must not reach any request.
- **HCR-10** (deterministic) A review `wrong` must be counted as answered once, by its section, its date and its
  comment, so that a new review of the same section asks its unit again (HCR-4).
- **HCR-11** (deterministic) `generate` must fail with an error naming `.retrodoc/edits.json` when the file cannot
  be read or is of a later format, before any LLM call, and go on with a warning and no review when
  `.retrodoc/reviews.json` cannot be read (RHI-14).
- **HCR-12** (deterministic) The fingerprint of a unit must leave its edits and its reviews out, the reviews `wrong`
  it answered (HCR-10) aside, so that a saved run with no review `wrong` left to answer asks no unit again (RER-1).

## Scenarios

- **HCR-H1** (happy; HCR-2, HCR-4, HCR-6, HCR-8) Given a use case marked `wrong` with the comment "the customer pays
  on delivery, not online", when `generate` runs, then the use cases of its feature are asked again, the request holds
  the slug of the use case and the comment, and no other unit is asked.
- **HCR-H2** (happy; HCR-1, HCR-5) Given a feature whose name a teammate corrected, when a later `generate` asks the
  features of its domain again because an entry point changed, then the request holds the corrected name as the one to
  keep.
- (HCR-4, HCR-10) Given a use case marked `wrong` and asked again by a run, when `generate` runs again with no other
  change, then no LLM call is made; when the team marks it `wrong` again with another comment, then the next run asks
  its feature again.
- (HCR-5, HCR-12) Given an edited use case and no other change, when `generate` runs, then no LLM call is made.
- (HCR-3) Given a feature `validated` and a sibling feature marked `wrong`, when the features of their domain are asked
  again, then the request holds the validated feature as the one to keep and the comment of the other.
- (HCR-7) Given two features each with an edited use case, when one feature's entry point changes, then only its
  request holds a correction, its own.
- (HCR-9) Given a domain reviewed `to_review` and an edit of a feature no saved result holds, when the domains and the
  features are asked again, then no request holds a correction.
- (HCR-11) Given an `edits.json` holding invalid JSON, when `generate` runs, then it fails naming the file and the LLM
  is never called; given a `reviews.json` holding invalid JSON instead, then it warns of it and asks no unit again.
- (HCR-6) Given a domain asked in two slices and a feature of the second slice marked `wrong`, when `generate` runs,
  then only the second slice is asked again.

## Non-goals

- No write to `.retrodoc/edits.json` nor `.retrodoc/reviews.json` by a run (SED-11, REV-8): the run does not propose
  an edit, drop one nor mark one answered there.
- No change of the outdated rule: a section written again under its edit's guidance still makes the edit outdated as
  "rewritten" (SED-34), and the team keeps or drops it.
- No check that the model followed a correction: what it writes depends on the model, measured on a saved run, not
  required.
- No correction read by the other passes (steps beyond the use case call, business rules, glossary, confidence,
  reconciliation).
- A review `wrong` outdated by the rewrite it asked keeps guiding the later rewrites of its unit until the team
  changes or removes it.
