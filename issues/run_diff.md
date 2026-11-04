---
priority: medium
depends_on: [run_snapshot_at_generate]
why: 'the difference half of the milestone: says which sections a run added, removed or degraded'
---
# Show the difference between two runs on a history page

# Goal
Nothing tells which sections a `generate` added, removed or made worse. Expected: a page `/history` of `retrodoc ui`
lists the saved runs and compares two of them (the last two by default): the domains, features and use cases added
and removed, and those whose confidence or business language fell, each linked to its page when it is still in the
run.

# Findings
* 2026-11-04: no page or command compares two runs; the incremental `generate` rewrites the artifacts in place.
* Chosen with the user on 2026-11-04: the difference is a web page (`/history`), not a CLI command.

# Approach
1. The list of the runs of the snapshots (date, commit), two of them picked by a form, `?from=` and `?to=`.
2. The difference by section id: added, removed, degraded (a score lower than in the other run, by how much), and
   set apart or no longer set apart.
3. A threshold under which a fall is not shown, if the scores of two runs of the same code jitter: measure first on
   two snapshots of the same commit.

# Specs
* A new spec of the history page, or new requirements of the spec of the run history.

# Milestone
Run history

# Resources
* `crates/retrodoc-web/src/` (the pages, the "Audit" group of the header), `crates/retrodoc-mcp/src/read_model.rs`.
* The spec of the run history written by `issues/done/run_snapshot_at_generate.md`; ADR 0039.

# Hints
* The repeated runs of a benchmark series (the saved runs of the measures, same code, same inputs) show how much the scores
  jitter between two runs of the same code, with no LLM call.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
A page `/history` of `retrodoc ui` lists the saved snapshots and compares two of them (the last two by default, else
`?from=`/`?to=`): sections added, removed, degraded (confidence or business language fallen by at least a threshold),
newly or no longer set apart, each linked to its page when the served docs hold it. Out of scope: a CLI command, the
sections that got better and the trends of the figures (`issues/web_run_trends.md`). Spec: `specs/history-page.md`.

## Plan
1. [x] Difference of two snapshots: `retrodoc_mcp::history` names each loaded snapshot by its file and computes the
   difference of two (added, removed, degraded over the threshold, set apart or no longer); verified by unit tests on
   built snapshots; makes true HPG-6 to HPG-11, HPG-13 at the level of the model (~400 lines). (autonomous)
2. [ ] The page `/history`: the snapshots read at the start of `ui`, the list of the runs, the form, the defaults and
   the unknown run, the sections linked to their pages, fewer than two runs, the link in the "Audit" group of the
   header (WEB-67 amended), the English and French texts; verified by router tests over a temp repository; makes true
   HPG-1 to HPG-15 and sets the spec `accepted` (~700 lines).
3. [ ] Docs of the whole issue and fixes of the `high` review; verified by the tests of each fix.

## Decisions
- Mode: autonomous
- Base commit: 03d742f316258f8a29438fedd7c2268440166c01
- Threshold of a fall (HPG-8): 0.2, the p90 of the measured jitter of the confidence (chosen at validation).
- Jitter measured on 2026-11-04 with no LLM call on the three hidden runs of the same code (f95ada6) of linkding and
  lobsters, sections matched by id (about a quarter of them across independent runs): confidence fall median 0 to
  0.075, p90 0.12 to 0.2, p95 0.15 to 0.33, max 0.33; business language median 0, p95 up to 0.3. An incremental
  rerun keeps the scores of the sections it does not regenerate, so the jitter only touches those it regenerates.

## Deliverable 1: difference of two snapshots
- done: `history::load` names each snapshot by its file (`SavedRun`); `history::diff` gives the sections added,
  removed, degraded (a fall of at least `FALL_THRESHOLD`, 0.2), newly and no longer set apart.
- red: HPG-6, HPG-7, HPG-8, HPG-9, HPG-10, HPG-11, HPG-13: with `diff` returning nothing, `left: []` against
  `right: ["auth", "refunds/refund", "pay/cancel"]`, `[("pay/large", [Confidence]), ...]`, `["pay/new"]`, and the
  reversed run not seen as different.
- checks: project-check ok; deliverable size 381/1000.
- Code review: no findings (`low`).
- not verified: no page shows the difference yet (deliverable 2).
