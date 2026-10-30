---
rank: 600
why: 'low: follow-up of the closed split by family (2026-10-14): one context still over 150 files, action names taken for families, the names unread; nothing before it depends on it, and its spread measure is cheaper after `sampling_temperature_for_steadier_runs`'
---
# Finish the split of the contexts: the one still too big, the action names, the names read

# Goal
`issues/done/split_contexts_by_family_not_by_layer.md` cut the large context of a layered application by business
family. Three things were left unchecked or unsolved. Close them so that every context fits one run and its
name says what it holds, and so that the map is steady enough to trust.

# Findings (first real run of the split by family on the large Rails application, 2026-10-14, hosted `deepseek-chat`)
* 34 contexts, the largest 167 files (199 before). One context stays over 150 files after the two levels of split;
  `contexts` warns and a run takes it whole. The cause was not looked at: the part may have too few groups and
  folders to cut (fewer than three), or the answer gave it back whole.
* `family_groups` still takes for a family some action names that repeat in several layers (`validate`, `approve`,
  `restart`, `reissue`; 8 of the 13 families found under the workflow folder of that application, measured with
  `context_map_measure --families`). The five real business families are among the 13. The split call sees
  these in its listing with their sample and may place them well, but nothing checks it.
* Only the largest contexts were read after the run. The other names were not read against their content, and
  there was a single run, so the spread from one call to another is not known (earlier builds of the same map
  had the support going from 14% to 38% of the files).

# Approach
Measure first, with no LLM call where possible.
The saved map of 2026-10-13 is lost (the run directory is empty, see the ledger of measures (`CLAUDE.md`)): steps 1 and 2 first need one
`contexts` run of the application (the map alone, about 5 calls, about $0.01), which step 3 then repeats.
1. Find why the largest context was not cut: its groups and folders on the saved map (`contexts.yaml`), and the
   number of listed entries the split call got for it. If it is too few entries to cut, list its files by
   family or by sub-folder one level deeper, or accept the context as is above the limit.
2. Action names: on the saved run, count for each family found how many layer folders hold it and whether a
   `workflow_actions`-like folder is the one that repeats it; try a rule that drops a name which shows in the
   action folder only, and check that the five real families stay (`context_map_measure --families`).
3. Read the names of the 34 contexts against their files (a person, or a check by the call that names a part),
   and run `contexts --force` two or three times to measure the spread of the map; report it in the issue.

# Milestone
A large application, one bounded context at a time

# Resources
* `issues/done/split_contexts_by_family_not_by_layer.md` (the work, the measure and the real run)
* ADR 0029, `crates/retrodoc-pipeline/src/contexts/` (`groups.rs` `family_groups`, `mod.rs` `split_context`)
* `crates/retrodoc-pipeline/examples/context_map_measure.rs` (`--families`, `FAMILIES_UNDER=<folder>`)

# Specs
* `specs/context-split.md` (CSP)

# Hints
* A context already documented must stay as it is: offer a new map with `retrodoc contexts --force` only.
* The application is confidential: describe it generically.
* The same call is not steady (hosted model, no fixed seed); one run settles nothing, see
  `sampling_temperature_for_steadier_runs` which may come first.

# Tracking

Plan: validated

## Understanding
Make the contexts map trustworthy on the large Rails application: know why one context stays over 150 files and
cut it (or accept it), stop action names from being taken for families, read the names and measure the spread.
Means: the warning itself says why a context stayed whole (CSP-4), so the run diagnoses it with no extra tooling.

## Plan
1. [x] Diagnostic warning: a context left over the limit is warned with its slug, file count and reason; the
   existing tests trace CSP-1/2/3/5; verified by `FakeLlm` tests in `contexts/tests.rs`; makes true CSP-1 to CSP-5.
2. [ ] Measure, steps 1-2 (no code): copy the saved complete run to `~/retrodoc-runs/contexts_residual-<commit>-1`,
   one `retrodoc contexts` (~5 calls, ~$0.01), read the warning and `contexts.yaml`; `context_map_measure
   --families` (`FAMILIES_UNDER=` the workflow folder) for the action names; findings in the issue and the ledger.
3. [ ] Residual size, by the diagnosis of 2: list the part one level deeper (by family or sub-folder) when it has
   too few entries, or accept it above the limit (then CSP-3/4 already cover it, no code); spec amended (CSP-6);
   verified by a `FakeLlm` test.
4. [ ] Action names: a rule in `family_groups` that drops a name shown only in an actions folder (rule shaped by
   the measure of 2); verified by a unit test and `context_map_measure --families` keeping the five real families;
   makes true CSP-7.
5. [ ] Names and spread (no code): `contexts --force` two more times (~$0.02), names of the contexts read against
   their files, spread (count, largest, support share, stable names) reported in the issue and the ledger.

## Decisions
- Mode: autonomous
- Base commit: a9d06bd
- Paid runs of deliverables 2 and 5 (~$0.03) approved up front.
- The agent reads the context names against their files (deliverable 5).
- Too few entries to cut (deliverable 3): list one level deeper, by family or sub-folder.

## Deliverable 1
- done: a context left over 150 files is warned of one by one (`context=`, `files=`, `reason=`); spec CSP accepted.
- checks: `just check` ok (tests, clippy, specs); docs `docs/crates/pipeline.md`; code review below.
- red: CSP-2, CSP-3, CSP-4: `no warning for <slug>` (4 new or extended tests; the code was written just before the
  tests, red checked by running the tests on the stashed code).
- Code review: no findings
- not verified: the warning on the real application (deliverable 2).
