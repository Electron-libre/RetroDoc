---
priority: high
why: 'the trends and the difference between two runs of the milestone both read the snapshots: nothing else of it can start before'
---
# Save a snapshot of the figures and the sections at each generate

# Goal
The debt report, `/audit` and `/debt` read the artifacts of the last run only: once a `generate` rewrites
`.retrodoc/cache/`, the state before it is lost, so nobody can tell whether the docs get better. Expected: each
`generate` that changes the docs leaves a small snapshot of the run (its figures and its sections with their scores)
that a later page reads to show the trends and the difference between two runs.

# Findings
* 2026-11-04, code at commit e5a5f2c: the only history is `.retrodoc/cache/usage.json` (`usage_log::record_run`, the
  last 20 commands, cost per pass, no quality figure); `build_report` computes the debt from the last artifacts at
  each read; `/audit` (`specs/audit-dashboard.md`) shows the last run only.
* The reconciliation with the project's docs is recomputed from the saved verdicts with no LLM call, so a `generate`
  can count its contradicted and undocumented items; the stale items mean nothing at the end of a `generate`
  (nothing changed since yet).
* Chosen with the user on 2026-11-04: every snapshot is kept (a few kB each, the trends need the long run), and none
  is written by a `generate` that changed no section nor figure (a no-op rerun).

# Approach
1. A snapshot per run: the `HEAD` commit, the end date, the overall confidence and business language, the entry point
   coverage, the debt counted by kind and domain (the kinds of `/debt` known at the end of a `generate`, not stale),
   the cost of the run, and per section (domain, feature, use case) its confidence, business language, narrative
   check and set apart reasons.
2. Where: one file per run under `.retrodoc/cache/` (a `history/` folder named by date), or one appended file;
   compare the cost of a read of all of them by the trends page.
3. A no-op rerun (same sections and figures as the last snapshot) writes none.

# Specs
* A new spec of the run history (requirements of the snapshot, its content, when none is written), written at
  `issue-workflow` step 1.

# Milestone
Run history

# Resources
* `crates/retrodoc-pipeline/src/usage_log.rs` (the history of the cost), `crates/retrodoc-pipeline/src/report/`
  (`build_report`, `DebtReport`), `crates/retrodoc-mcp/src/audit` and `crates/retrodoc-mcp/src/debt` (the figures of
  `/audit` and the debt items), `crates/retrodoc-cli/src/commands/generate/`.
* `specs/audit-dashboard.md`, `specs/debt-view.md`; ADR 0039.

# Hints
* A rerun that calls no LLM must stay a no-op on the artifacts (smoke-test criterion): the snapshot must not break it.
* No LLM call is needed: a saved run of the measures gives the inputs of the tests and a first snapshot.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
At the end of each successful `generate`, write a JSON snapshot (figures of `/audit`, debt per kind and domain without
stale, cost, scores of every domain, feature and use case) as one file per run under `.retrodoc/cache/history/`,
skipped when equal to the last one but for commit, date and cost. Out of scope: any page reading them (the two other
issues of the milestone). Means: the snapshot is built in `retrodoc-mcp` (module `history`) from the read model, the
CLI writes it after `usage::finish` (ADR 0043).

## Plan
1. [x] Snapshot and store: `retrodoc_mcp::history` builds the snapshot from `GeneratedDocs` and a `RunUsage`, saves it
   (one file per run, unique name, no-op skip) and loads the list; verified by unit tests on fixture docs; makes true
   RHI-2 to RHI-9, RHI-11, RHI-13 (~550 lines). (autonomous)
2. [ ] Write at generate: the CLI writes the snapshot after a successful, non dry-run `generate`, warning on failure;
   verified by end-to-end tests through the binary with the fake LLM (first run writes, rerun writes none, dry-run
   and failure write none, `--force` keeps them); makes true RHI-1, RHI-10, RHI-12 and sets the spec `accepted`
   (~350 lines).

## Decisions
- Mode: autonomous
- Base commit: 78b0a3fc459f8111567cc0b10420fa4b73080bab
- Storage: one file per run under `.retrodoc/cache/history/` (ADR 0043), chosen at validation.

## Deliverable 1: snapshot and store
- done: `retrodoc_mcp::history` builds the snapshot from the read model (same report, coverage and debt as `/audit`),
  saves one file per run with a unique name, skips a no-op, loads the list oldest first; `DebtKind::key` moved from
  the web crate to name the kinds.
- red: RHI-9: with the skip removed, `a_snapshot_equal_to_the_last_but_for_commit_date_and_cost_is_not_saved` fails
  with `left: Some(".../history/20261105T090000Z.json") right: None` (checked by mutation: tests and code were written
  in one go, the other tests were not seen red).
- checks: project-check ok (fmt, clippy, tests, specs); deliverable size 748/1000.
- Code review: no findings (`low`).
- not verified: nothing writes a snapshot yet (deliverable 2).
