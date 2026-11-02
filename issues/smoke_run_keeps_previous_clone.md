---
priority: medium
depends_on: []
why: 'protects the saved runs the planned measures read; the copy rule of the skill covers it until then (code review of the ledger issue, 2026-10-27)'
---
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
* `issues/done/measure_ledger_and_shared_measure_plan.md`

# Hints
* `smoke.rs` is a project skill, not synced: it can change here.

# Tracking

Plan: validated

## Understanding
`smoke.rs run` must never delete or overwrite a run the ledger still lists, and the disk use must stay bounded.
Out of scope: the existing fixed-name runs (`retrodoc-smoke-<repo>`), left untouched; the other scripts writing runs.
Means: candidates (a) + (c): one fresh name per run, and pruning of only the unlisted older runs of the same repo.

## Plan
1. [x] Fresh run name and bounded pruning: each run is named `retrodoc-smoke-<repo>-<short RetroDoc commit>-<n>`
   (`n` the first free number; clone dir plus `.run1.log`, `.run2.log`, `.report.txt`), so nothing is overwritten;
   before cloning, the older runs of that repo under the new scheme are removed unless `LEDGER.md` names them (whole-name
   match) or it is the most recent one (its ledger line may not be written yet); no ledger means nothing is removed;
   the legacy fixed-name runs are never touched. An internal `prepare-run <repo name>` subcommand (like
   `llm-target`) prints the chosen name and the removed runs; `SKILL.md` drops the copy-by-hand rule and gives the
   new paths; verified by new cases of `test_smoke.rs` on a temporary `RETRODOC_RUNS_DIR` (fresh name, numbering,
   listed run kept, latest unlisted kept, older unlisted removed, no ledger keeps all, legacy dir kept).

## Decisions
- Mode: autonomous
- Base commit: af50fa589d5f6e48f5b160107f298064afced4f8
- Old unlisted runs: removed, except the most recent (1 a).
- Legacy fixed-name runs: never touched (2 a).

## Summary of deliverable 1
done: `smoke.rs` gives each run a new name and prunes only the unlisted older runs of the repo (latest kept); `SKILL.md` updated.

| check | result |
|---|---|
| tests (`test_smoke.rs`) | pass |
| `just check` | pass |
| docs | `SKILL.md`, module doc of `smoke.rs`; nothing else cites the old layout |
| code review (`low`) | 1 finding, fixed |

red: none (no spec, harness): the 3 new cases failed with `usage: smoke.rs run <repo> | ...` (no `prepare-run`).

Code review: 1 findings
- unchecked exit status of `git rev-parse` (empty commit, a name never counted, the next clone fails): fixed, `die` before any removal.

not verified: a real `just smoke` run (needs the local LLM).
