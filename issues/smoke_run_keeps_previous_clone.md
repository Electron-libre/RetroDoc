# Keep a smoke run that a measure still reads

# Goal
`just smoke <repo>` deletes the previous clone of the same repository and overwrites its logs before it runs.
A run that the ledger of measures lists as a reusable artifact (`CLAUDE.md`) is lost when someone runs a smoke
test of the same repository without copying it first; the skill only asks the agent to copy it by hand.
Expected outcome: a smoke run never deletes a run the ledger still needs.

# Findings (2026-10-27)
* `smoke.rs` names the clone `retrodoc-smoke-<repo dir name>` and its logs `.run1.log`, `.run2.log`,
  `.report.txt`, and removes the clone with no check before cloning again.
* 63 of the 130 entries of the runs directory were empty when the ledger was backfilled, the context map of
  the large Rails application among them: runs are lost in practice, though not by this path alone.
* The code review of `measure_ledger_and_shared_measure_plan` (level high) raised it; that issue kept a text
  rule in the skill `smoke-test`.

# Approach
1. Candidates (can be combined): (a) name each clone after its commit and a run number instead of reusing
   one name; (b) move the previous clone and logs aside (`.prev`) instead of deleting them; (c) refuse to
   delete a clone the ledger lists as kept (needs a parseable list in the ledger).
2. Keep the disk use bounded: a delivery_router clone is under 1 MB, a large application about 100 MB.

# Resources
* `.claude/skills/smoke-test/smoke.rs`, `test_smoke.rs`, `SKILL.md` (section "Ledger of measures")
* `issues/measure_ledger_and_shared_measure_plan.md`

# Hints
* `smoke.rs` is a project skill, not synced: it can change here.
