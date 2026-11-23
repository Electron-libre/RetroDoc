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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-23

## Understanding
The readers stop calling `retrodoc-pipeline` for the reconciliation, the debt report, `domain_models` and `Bm25`: the
deterministic code moves to `retrodoc-core` (ADR 0053 draft), the split of the collected docs into sections to
`retrodoc-ingest`, the LLM judge stays in the pipeline. Out of scope: the run history and usage log (own issue). No
behaviour change, no spec (pure refactor).

## Plan
1. [x] Bm25 in core: move `bm25.rs` to `retrodoc-core`, the pipeline, the bench and the MCP crate import it from
   there; ADR 0053 draft committed; verified by the moved tests and `project-check`; makes true the Goal for `Bm25`. (autonomous)
2. [ ] Split the tests of the reconciliation: the judge's tests to a file of their own (`reconcile/tests.rs` is 1,166
   lines, over the file size limit), no change of behaviour; verified by the same tests passing.
3. [ ] Reconciliation in core: its types, `items`, `candidates`, `Reconciliation`, `reconcile` and the `Judgements`
   type with its load move to `retrodoc_core::reconcile`, the section split (`corpus`) to `retrodoc-ingest`; `judge`
   stays in the pipeline; `generate`, the MCP crate, the web tests and the CLI repointed; verified by the moved
   tests, `cache_format_sample` (format unchanged) and `project-check`.
4. [ ] Debt report in core: `report/` (`build_report`, `DebtReport`, `domain_models` and their types) moves to
   `retrodoc_core::report`, the readers repointed; verified by the moved tests and `project-check`.
5. [ ] Readers checked and docs: no `retrodoc_pipeline::{reconcile, report, domain_models, Bm25}` left in the readers
   (grep), ADR 0053 accepted, `docs/crates/` and `docs/ARCHITECTURE.md` updated; a rerun of `generate` on a saved
   run copy calls no LLM and rewrites no file; verified by that rerun and `project-check`.

## Decisions
- Mode: autonomous
- Base commit: bfd82126f0c48389605edbaba3299f170192978b
- Q1 (where the section split goes): `retrodoc-ingest`, returning the core `Section`; `reconcile` takes the sections.
- Q2 (where the report and the reconciliation types go): `retrodoc-core` now.

## Summaries

### Deliverable 1
- done: `bm25.rs` moved to `retrodoc_core::bm25` (`has_content_words` public, `#[must_use]`); the pipeline imports it
  privately, the bench, its `placement_replay` example and the MCP search import it from core; ADR 0053 draft.
- checks: tests, lint, specs, docs (`core-ingest.md`, `render-mcp-cli.md`) green in `project-check`.
- red: none (move with no change of behaviour; the moved tests pass in core).
- not verified: `retrodoc-web --test ask_page` failed once in a first `project-check`, then passed alone and in the
  rerun (flaky, unrelated to the move).
- Code review: no findings
