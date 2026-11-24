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
