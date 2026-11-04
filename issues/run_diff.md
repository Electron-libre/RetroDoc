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
