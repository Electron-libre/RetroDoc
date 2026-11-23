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
2. [x] Verdicts readable without the judge's prompt: `reconciliation.yaml` records the two parts the pass's
   fingerprints start with (`shown_rule`, `request`), `Judgements::of` fingerprints from them, the same fingerprint as
   before; an older file is rewritten once by the next `generate`, with no LLM call; spec
   `docs-reconciliation` amended; verified by a test of the recorded parts and of the rewrite. (autonomous)
3. [x] Split the tests of the reconciliation: the judge's tests to a file of their own (`reconcile/tests.rs` is 1,166
   lines, over the file size limit), no change of behaviour; verified by the same tests passing. (autonomous)
4. [x] Reconciliation prepared in place: the judge out of `reconcile/` into a pipeline module of its own, `corpus` to
   `retrodoc-ingest` (returning the `Section` of the reconciliation), `reconcile/` reaching only paths that resolve
   in core too; verified by the tests and `project-check`. (autonomous)
5. [x] Exact move of `reconcile/` to `retrodoc_core::reconcile` (`deliverable-size` counts only an exact rename once),
   `generate`, the MCP crate, the web tests and the CLI repointed; verified by the moved tests,
   `cache_format_sample` and `project-check`. (autonomous)
6. [x] Debt report in core and readers checked: `report/` prepared in place then moved exactly to
   `retrodoc_core::report`; no `retrodoc_pipeline::{reconcile, report, domain_models, Bm25}` left in the readers
   (grep); ADR 0053 accepted; `docs/crates/`, `docs/ARCHITECTURE.md`, `CLAUDE.md` updated; a rerun of `generate` on a
   saved run copy calls no LLM and rewrites no file; verified by that rerun and `project-check`. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: bfd82126f0c48389605edbaba3299f170192978b
- Q1 (where the section split goes): `retrodoc-ingest`, returning the core `Section`; `reconcile` takes the sections.
- Q2 (where the report and the reconciliation types go): `retrodoc-core` now.
- Q3 (how core applies the verdicts without the judge's prompt): `reconciliation.yaml` records `shown_rule` and
  `request`; core fingerprints from them. Plan re-cut to 6 deliverables (validated by Cedric Brancourt on 2026-11-23):
  `deliverable-size` detects only exact renames, so each move is prepared in place first.

## Summaries

### Deliverable 1
- done: `bm25.rs` moved to `retrodoc_core::bm25` (`has_content_words` public, `#[must_use]`); the pipeline imports it
  privately, the bench, its `placement_replay` example and the MCP search import it from core; ADR 0053 draft.
- checks: tests, lint, specs, docs (`core-ingest.md`, `render-mcp-cli.md`) green in `project-check`.
- red: none (move with no change of behaviour; the moved tests pass in core).
- not verified: `retrodoc-web --test ask_page` failed once in a first `project-check`, then passed alone and in the
  rerun (flaky, unrelated to the move).
- Code review: no findings

### Deliverable 2
- done: `Judgements` records `shown_rule` and `request`; `of` fingerprints with them (none when absent);
  `Candidate::fingerprint_with`; the pass reuses with its own values and rewrites an older file once, no call;
  spec `docs-reconciliation` sorted under Behaviour and Contract, REC-17 added, back to `accepted`.
- checks: tests, lint, specs, docs (`pipeline.md`, `ARCHITECTURE.md` §7) green in `project-check`.
- red: REC-17: `struct Judgements has no field named shown_rule` / `no method named fingerprint_with` (compile).
- Code review: no findings
- file size: `reconcile/tests.rs` 1,240 lines (was 1,166), split by deliverable 3.
- not verified: none.

### Deliverable 3
- done: the matching tests of the reconciliation moved to `reconcile/matching_tests.rs` (465 lines, they move to core
  with `mod.rs`); `reconcile/tests.rs` keeps the judge's tests (779 lines), which stay in the pipeline. The smaller
  part was moved: `deliverable-size` counted 1,555 lines the other way round.
- checks: tests (32 reconcile tests, as before), lint, specs green in `project-check`; docs n/a (no doc names the
  test files).
- red: none (no change of behaviour).
- Code review: no findings
- not verified: none.

### Deliverable 4
- done: `reconcile/mod.rs` holds the verdicts (`Judgements`, `Judged`, `JudgedSection`, `Answer`, their load and
  save in `CacheError`, `Candidate::fingerprint_with` over `hash_content`) and reaches only `retrodoc_core` paths;
  the judge is the module `reconcile_judge` (`reconcile/judge.rs`, its tests `reconcile/tests.rs`), with `recorded`,
  `fingerprint` and `to_judge` as functions; `corpus` in `doc_corpus` with its test; `reconcile` takes the sections;
  `generate` and the MCP read model repointed; the matching tests build their sections by hand.
- checks: tests, lint, specs, docs (`pipeline.md`) green in `project-check`.
- red: none (no change of behaviour; the fingerprint is computed the same, the reuse tests pass).
- Code review: no findings
- not verified: none.

### Deliverable 5
- done: `reconcile/mod.rs` and `matching_tests.rs` moved exactly to `retrodoc_core::reconcile` (core names itself
  `retrodoc_core`, `extern crate self`); `doc_corpus` moved to `retrodoc-ingest`; the pipeline's `reconcile` module is
  the judge (`reconcile/judge.rs`); the report, `generate`, the CLI docs, the MCP crate, the web and cache format
  tests import the reconciliation from core.
- checks: tests (`cache_format_sample` included), lint, specs, docs (`core-ingest.md`, `pipeline.md`) green in
  `project-check`; 181 changed lines (exact renames).
- red: none (move with no change of behaviour).
- Code review: no findings
- not verified: none.

### Deliverable 6, first commit (the report prepared in place)
- done: `report/` reaches only `retrodoc_core` paths (coverage, domain map, set-aside items, `one_line`), so it
  moves exactly next.
- checks: tests, lint, specs green in `project-check`; docs n/a (no path changed yet).
- red: none (no change of behaviour).
- Code review: no findings
- not verified: none.

### Deliverable 6, second commit (the report moved)
- done: `report/` moved exactly to `retrodoc_core::report`; the pipeline no longer exports it; the CLI and the MCP
  crate import the report and `domain_models` from core. Grep of `retrodoc-mcp`, `retrodoc-web`, `retrodoc-agent`,
  `retrodoc-cli`, `retrodoc-bench`: no `retrodoc_pipeline::{reconcile, report, domain_models, Bm25}` left but the
  judge, which `generate` calls (`retrodoc_pipeline::reconcile`). ADR 0053 accepted; `core-ingest.md` holds the
  report's detail, `pipeline.md`, `ARCHITECTURE.md` §2 and §4, `CLAUDE.md` updated.
- checks: tests, lint, specs, docs green in `project-check`; 52 changed lines.
- rerun: a session copy of the saved run `reader_types_in_core-rerun/`, `generate --no-confidence` with a bogus key:
  no LLM call, the 141 files of `.retrodoc/` and `docs/` of identical content (ledger line added).
- red: none (move with no change of behaviour).
- Code review: no findings
- not verified: the one-time rewrite of an older `reconciliation.yaml` on a real run: the saved run has no project
  docs, so `reconcile` is off there; the tests of REC-17 cover it.
