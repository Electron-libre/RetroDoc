---
priority: low
depends_on: [reader_types_in_core]
why: 'the readers call the pipeline to compute the reconciliation and the debt report; needed before the docs crate'
---
# Take the computations the readers make at read time out of the pipeline

# Goal
The read model computes, each time it loads the docs, the reconciliation with the collected docs (`reconcile`),
the debt report (`build_report`, `DebtReport`), the domains with their features (`domain_models`), and the search
ranks with `Bm25`, all functions of `retrodoc-pipeline`. Expected: the readers no longer call the pipeline for them,
either because `generate` saves the result as an artifact, or because the computation lives in a module the readers
may depend on.

# Findings
* 2026-11-19: `retrodoc-mcp` uses `reconcile::{reconcile, Judgements, Reconciliation, ItemVerdict, Status, ItemKind,
  Contradiction}`, `report::{build_report, DebtReport, Section, WeakSection, MarkedWrong, OutdatedReview,
  ChangedFile}`, `domain_models`, `Bm25`.
* `reconcile` needs `retrodoc_ingest::existing_docs::ExistingDoc` and `signals::doc_sections`, and `Bm25`; core
  does not depend on `retrodoc-ingest`.
* The debt report is built from the docs as shown, edits included (SED-39, SED-40), so it cannot be saved whole at
  `generate` time: an edit made after the run changes it.
* `reconcile/` is about 1,000 lines without its tests, `report/` about 960.

# Approach
1. Sort each computation: saved as an artifact at `generate` (the verdicts of the reconciliation are already in
   `reconciliation.yaml`), or moved with its types.
2. Candidates for the moved ones: `retrodoc-core` (`Bm25` and the report have no dependency on ingest), or the docs
   crate of `docs_crate_out_of_the_mcp_crate` (allowed to depend on `retrodoc-ingest`).
3. An ADR for the choice; check a rerun of a saved run calls no LLM and rewrites no file.

# Milestone
The readers of the docs out of the pipeline

# Resources
* ADR 0033 (reconciliation), ADR 0051, `specs/docs-reconciliation.md`, `specs/debt-view.md`, `specs/section-edit.md`.
* `docs/crates/pipeline.md`, `docs/crates/render-mcp-cli.md`.

# Hints
* Likely over the size limit of `skill:issue-workflow`: cut at step 1 (one issue per computation).
