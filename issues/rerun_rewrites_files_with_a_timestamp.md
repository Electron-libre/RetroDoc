---
priority: low
depends_on: []
why: 'priority low (user, 2026-10-16): the rerun of the same smoke writes 3 files for a `generated_at` stamp; cosmetic for the smoke criterion'
---
# Stop an unchanged rerun from rewriting files that hold a timestamp

# Goal
Priority: low. A second `generate` that reuses everything from the cache still writes files: it must write
none, so that the smoke test criterion "the rerun is a no-op" holds on a large application.

# Findings (smoke run of 2026-10-15, a large Rails application, local model)
* The rerun made no LLM call ("everything was reused from the cache") but reported "3 file(s) written to docs".
* The diff shown in the log was a `generated_at` timestamp (next to a `commit` field) that changed between
  the two runs.
* Which three files, and whether other generated files carry a timestamp, is not known.

# Approach
1. Find the three files and the code that stamps them (`retrodoc-render`).
2. Candidates: keep the previous timestamp when the rest of the content is the same; or stamp only when
   something else changed.

# Resources
* `crates/retrodoc-render/`, `docs/crates/render-mcp-cli.md` (idempotence)
* `.claude/skills/smoke-test/SKILL.md` (criterion 3)

# Tracking

Plan: validated

## Understanding
The timestamp is not the cause: `plan.rs` already ignores `generated_at` when the metadata is the only change. The
rerun log of the smoke shows the two other files, the domain `README.md` and `llms.txt`: five
features of a sliced domain moved. On reuse, a slice takes back every previous feature holding one of its entry
points (or one from outside the domain), so a feature born in a later slice that holds an entry point of an earlier
one comes back earlier. Fix in the features pass: reused features keep their previous order (RER-12). Out of scope:
sorting in the render.

## Plan
1. [x] Keep the saved order of the features of a domain after reuse; verified by a `from_surface` test (two slices,
   placement of a first-slice entry point into a second-slice feature, rerun keeps the order) and `just check`;
   makes true RER-12.

## Decisions
- Mode: autonomous
- Base commit: b9a5c84039d832c2245e68a2e66eef85c2e1105b
- New features on a partial redo: the reused ones take back their saved order in the slots they hold, the new ones
  stay where their slice put them (1 a).

## Deliverable 1
- done: `keep_saved_order` puts the reused features of a domain back in their saved order (new ones stay in place);
  RER-12 added to `specs/rerun.md`; `docs/crates/pipeline.md` updated.
- red: RER-12: `the saved order is kept`, left `["a", "c", "b"]`, right `["a", "b", "c"]` (the order changed by the
  reuse also changed the print of the placement, which was asked again: 3 calls).
- not verified: the rerun of the smoke on the large application (local run of about 1 h, not redone).

## Review of the whole issue (high)
- Code review: 6 findings. Fixed: the partial save after each slice keeps the saved order too; only the saved
  features of the same domain count as saved; the slug lookup uses one map; the confidential repository name left
  this file (it stays in the history of 0da7eee). Not fixed: the render plan in the test (the render follows the
  feature order, RER-8 covers its idempotence); the order put back where reused features merge rather than after
  (one function on both save paths is enough, `restricted` must keep taking back the placed entry points).
