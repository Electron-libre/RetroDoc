---
priority: medium
why: 'the edits survive a rerun but teach it nothing, so a corrected section can be rewritten wrong again; not needed for the reading itself'
---
# Let the human edits and reviews guide the next run

# Goal
A user asked whether the edits made to the docs are kept from one run to the next, and whether they guide the next
run. They are kept, but the next `generate` does not read them: a section corrected by hand, or marked wrong with a
comment, is generated again from the code alone, and a rewrite only makes the edit or the review outdated. Expected:
an answer to the question written in the user docs, and a `generate` that takes the human corrections as input, so
that what a human fixed is not lost or contradicted by the next run.

# Findings
* Kept: `generate`, `--force` included, neither deletes nor rewrites `.retrodoc/edits.json` nor
  `.retrodoc/reviews.json` (`specs/section-edit.md` SED-11, `specs/section-review.md` REV-8); the edits are an
  overlay the readers apply (ADR 0045), still shown when outdated (SED-37).
* Not guiding: no module of `crates/retrodoc-pipeline` reads the edits or the reviews (grep, 2026-11-24). A run that
  rewrites an edited section makes its edit outdated, "rewritten" (SED-34), and the team must merge it by hand
  (non-goal of SED: no merge of an outdated edit with the new generated text).
* The prompt is part of the fingerprints (ADR 0037): a correction given to a pass changes its fingerprint, so it
  reruns the calls it touches, and only those if the input is scoped per section.

# Approach
1. Write the answer to the question where the user reads it (README or the web page of an edit): kept, shown,
   outdated on a rewrite, not used by the run.
2. Decide what a correction feeds, per kind:
   1. an edited name or description given to the pass that writes the section, as the reference to keep unless the
      code contradicts it;
   2. a review `wrong` with its comment given to that pass as what to fix;
   3. a review `validated` kept as is: its section is not rewritten while its fingerprint inputs are unchanged.
3. Measure on a saved run of the default smoke repository before and after: how many edited sections a rerun keeps
   in substance, how many calls the corrections rerun, and their cost.

# Specs
* `specs/section-edit.md` (its non-goals "no change to the generated artifacts" and "no merge" to revisit),
  `specs/section-review.md`, `specs/rerun.md`; new requirements for the passes that read the corrections.

# Milestone
Read by someone taking an application over

# Resources
* ADR 0037, 0044, 0045; `docs/crates/pipeline.md`, `docs/crates/docs.md`.
* `crates/retrodoc-docs/src/edits/`, `crates/retrodoc-docs/src/reviews/`.

# Hints
* The pipeline does not depend on `retrodoc-docs`: reading the stores from a pass needs their loader moved or a new
  crate edge, an ADR decision.
* An edit must stay the user's text: the run may propose, never overwrite `.retrodoc/edits.json`.
* Measures are paid: read the ledger first and ask before a run.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-24

## Understanding
`generate` gives the domains, features and use cases passes the edits and reviews of the sections each unit writes
(edit = text to keep, `wrong` + comment = what to fix, `validated` = keep), by slug; only a `wrong` review asks its
unit again, once. The CLI loads the stores and hands plain data to the pipeline (no new crate edge, ADR 0056). Out:
writing to `edits.json`/`reviews.json`, other passes, any change of the outdated rule. Spec: `specs/human-corrections.md`.

## Plan
1. [x] Corrections and use cases: `Corrections` type (`retrodoc-pipeline/src/corrections.rs`: per-section input, the
   prompt part, the set of `wrong` reviews answered in `fingerprints.json`), loaded by `generate` from the two stores
   (fails before any call on a bad file), given to the use cases pass; README answers "are my edits kept, do they
   guide the next run"; verified by pipeline tests with a fake LLM and a CLI test for the bad file; makes true HCR-1
   to HCR-5, HCR-7 to HCR-12 for use cases, HCR-6 (use case), HCR-H1 (~900 lines). (autonomous)
2. [x] Features: the corrections of the features of a domain in its call (the slice holding them), new code in
   `features/from_surface/corrections.rs` (`from_surface.rs` is over the file size limit); verified by pipeline
   tests; makes true HCR-6 (feature), HCR-H2, the slice scenario (~450 lines). (autonomous)
3. [x] Domains: the corrections of the domains in the domains call; verified by pipeline tests; makes true HCR-6
   (domain), HCR-9 scenario; spec `accepted` (~300 lines). (autonomous)
4. [ ] Measure: on a saved run of the default smoke repository, a few edits and `wrong` reviews, then `generate`:
   calls rerun, cost, corrected sections kept in substance (slug and text); entry in `docs/findings.md` and the
   ledger. Paid DeepSeek run: asked before it starts (~60 lines).

## Decisions
- Mode: autonomous
- Base commit: 944963231fee83b2373ce5cd7ce8d83706646ab6
- An edit does not ask its unit again: it guides the next rewrite the unit gets for another reason (HCR-5).
- A review `validated` is only shown to the model as the text to keep, never put back by the code.
- A review `wrong` of a domain asks the domains call again, cascade included; its cost is measured in deliverable 4.
- HCR-11 aligned on RHI-14 (accepted): an unreadable `reviews.json` is warned of and gives no review, the run goes on;
  an unreadable `edits.json` still stops it before any call (deliverable 1).

## Summaries

### 1. Corrections and use cases
- done: `generate` reads the edits and reviews before `--force`, gives each feature's use cases call the corrections of
  its use cases; a review `wrong` asks its feature again once (`Fingerprints::answered`); tutorial answers the question.
- red: HCR-H1, HCR-2, HCR-4, HCR-6, HCR-8, HCR-10: use-cases calls unchanged after the review `wrong` (left
  `[1, 1, 1]`, right `[1, 1, 2]`); HCR-1, HCR-3: the forced rerun's request lacked the edited narrative; HCR-11: an
  unreadable `reviews.json` did not stop `generate` (test rewritten after aligning on RHI-14); HCR-5, HCR-9, HCR-12
  passed before the code (nothing guided the run yet).
- Code review: 1 finding. Not fixed: `load_use_cases` gives `None` both for no saved use case (a first run, the
  usual case) and for an unreadable file, so a warning would fire on every first run.
- not verified: the corrections on a real model (deliverable 4).

### 2. Features
- done: each slice of a domain's features call is given the corrections of the features the previous run saved with one
  of its entry points; a feature marked wrong asks that slice again once.
- red: HCR-H2, HCR-1, HCR-5, HCR-6: the request of `orders` lacked "Order goods"; HCR-4, HCR-6 (slice): `left: 0,
  right: 1` calls after the review `wrong` (tests written after the code, red shown by emptying the corrections); the
  binary test of a feature `wrong` added with them.
- Code review: no findings.
- File size: `features/from_surface.rs` (+18 lines of wiring, the logic in `from_surface/corrections.rs`) and
  `generate/mod.rs` (801, +1 argument) not split here: a split by responsibility is a refactor of its own.
- not verified: the corrections on a real model (deliverable 4).

### 3. Domains
- done: the domains call is given the corrections of the domains (the uncategorized one left out); a domain marked
  wrong asks it again once; spec `accepted`, non-goal for `domains_from = "contexts"`.
- red: HCR-2, HCR-4, HCR-6: `left: [1, 1, 1], right: [2, 1, 1]` (no domains call after the review `wrong`); HCR-1,
  HCR-5, HCR-6, HCR-9 (internal): did not compile, no corrections argument yet.
- Code review: no findings.
- File size: `generate/mod.rs` at 802 (+1 argument), not split here, as in deliverable 2.
- not verified: the cascade of a domain `wrong` on a real model (deliverable 4).
