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
2. [x] Measure, steps 1-2 (no code): copy the saved complete run to `~/retrodoc-runs/contexts_residual-<commit>-1`,
   one `retrodoc contexts` (~5 calls, ~$0.01), read the warning and `contexts.yaml`; `context_map_measure
   --families` (`FAMILIES_UNDER=` the workflow folder) for the action names; findings in the issue and the ledger.
3. [x] (accepted, no code: core group plus the shared workflow steps, not cut by family; the warning gives the reason) Residual size, by the diagnosis of 2: list the part one level deeper (by family or sub-folder) when it has
   too few entries, or accept it above the limit (then CSP-3/4 already cover it, no code); spec amended (CSP-6);
   verified by a `FakeLlm` test.
4. [-] (dropped: one false family of 3 files with hosted roles) Action names: a rule in `family_groups` that drops a name shown only in an actions folder (rule shaped by
   the measure of 2); verified by a unit test and `context_map_measure --families` keeping the five real families;
   makes true CSP-7.
5. [x] Names and spread (no code): `contexts --force` two more times (~$0.02), names of the contexts read against
   their files, spread (count, largest, support share, stable names) reported in the issue and the ledger.
6. [x] Docs of the whole issue and fixes of the `high` review: ADR 0029 and `docs/findings.md` §8 record the
   residue and the spread; a part no longer takes the slug of another big context; the reason given when no
   level is left is named as such; the reasons are logged at once.

## Decisions
- Mode: autonomous
- Base commit: a9d06bd
- Paid runs of deliverables 2 and 5 (~$0.03) approved up front.
- The agent reads the context names against their files (deliverable 5).
- After deliverable 2 (no residue with the local roles): the map is redone from roles computed by the hosted model
  (`roles --force` on a second copy), as on 2026-10-13; deliverables 3-5 work on that copy.
- Deliverable 3: the context of 180 files is accepted above the limit (no third split level).
- Deliverable 4: dropped, no rule for the action names.
- Too few entries to cut (deliverable 3): list one level deeper, by family or sub-folder.

## Deliverable 1
- done: a context left over 150 files is warned of one by one (`context=`, `files=`, `reason=`); spec CSP accepted.
- checks: `just check` ok (tests, clippy, specs); docs `docs/crates/pipeline.md`; code review below.
- red: CSP-2, CSP-3, CSP-4: `no warning for <slug>` (4 new or extended tests; the code was written just before the
  tests, red checked by running the tests on the stashed code).
- Code review: no findings
- not verified: the warning on the real application (deliverable 2).

## Deliverable 2 (measure, 2026-10-30, 45ac040, hosted `deepseek-chat`)
Copy of the complete local run of 2026-10-15 (its `roles.yaml`, 1,676 source files, 266 entry point files):
- `retrodoc contexts`: 2 calls, 13.2k tokens in, 3.3k out, 17 s, ~$0.01. 37 contexts, the largest 117 files, none
  over 150; one context split (7 parts). 179 citations of the answer dropped (unknown or placed twice), 7 contexts
  dropped as empty, 445 files in the support.
- `context_map_measure --families`, `FAMILIES_UNDER=app/workflows`: 22 files in no group, 0 families. With these
  roles the families of the workflow folder are entry point groups (`market`, `hybrid`, `consortium`...), not
  orphans; over the whole application, 17 families on 516 files in no group, mostly layer folders (`model`,
  `component`, `helper`), no action name.
- The lost map of 2026-10-13 came from other roles (hosted): 4 contexts split at the first level, one left at 167
  files, and the workflow families as orphans in a context of 121 files with no entry point file.
- So neither residue shows on this copy: the size and the action names depend on the roles the map starts from.

## Deliverable 2b (measure, 2026-10-30, 70546ad, hosted `deepseek-chat`, roles redone by it)
Second copy, `roles --force` (2 calls, 15.3k in, 1.3k out) then `contexts` (4 calls, 14.1k in, 2.5k out), ~$0.01:
- 34 contexts, 500 files in the support; three splits (8, 7 and 4 parts). One context left at 180 files, warned as
  `reason=split twice already`: the first part of a split at the second level, so no third call.
- Its 180 files: the `contract` group (102 files by name), the shared steps of the workflow layer (34 tasks, 17
  workflow actions, 7 rules: `approve`, `validate`, `restart`, `archive`… used by every family), and the context's
  own folders kept as the first part's fallback (37 files of `app/models/`). The families (`market`, `hybrid`,
  `consortium`…) are entry point groups with contexts of their own.
- Families under the workflow folder: 1 over 57 files in no group (`archive`, 3 files, an action name). The 13
  families of 2026-10-13 came from roles where the family folders of `processes/` had no entry point file.

## Deliverable 5 (measure, 2026-10-30, 4f28917, hosted `deepseek-chat`, same copy and roles as 2b)
Two more `contexts --force` (4 and 6 calls, ~$0.02). Spread over the three maps of the same roles (1,676 source
files; a file "follows" when it lands in the context of the other run that holds most of its context's files):

| run | contexts | support | largest | over 150 |
|---|---|---|---|---|
| 1 (2b) | 34 | 500 (30%) | 180 | 1 |
| 2 | 40 | 166 (10%) | 143 | 0 |
| 3 | 44 | 165 (10%) | 146 | 0 |

- Pairs 1-2, 1-3, 2-3: 57-70%, 57-81%, 66-75% of the files follow; 75%, 80%, 93% keep their support status;
  4 slugs in all three maps. The map is not steady: a context of 180 files in one run is cut under the limit in
  the next two, so the residue of deliverable 3 is a draw of the call, not a fixed shape.
- Names read against their files (run 3, 44 contexts): about 30 fit (the business families of the workflow layer,
  companies, users, folders, signatories, notifications, imports, PDF rendering, the shared workflow steps now a
  context of their own). Off: 5 technical contexts that are not business (UI components 92 files, configuration
  86, front-end controllers 11, the application base classes 15, a front-end plugin 1), 3 catch-alls whose name
  covers part of the files (unrelated models, UI actions, misc views; 100 files), and 6 contexts of one file.

## Deliverable 6 (docs and `high` review)
- done: ADR 0029, `docs/findings.md` §8, `docs/crates/pipeline.md`; review fixes: the slugs of the big contexts
  are reserved while a level is split, `no split level left` replaces `split twice already`, each context left
  whole is logged when it is, CSP-2 and a CSP-5 scenario reworded, the CSP-2 test pins its context.
- checks: `just check` ok.
- Code review (`low`): no findings
- red: CSP-4: the new slug test gave `["contracts", "tasks", "contracts"]` (two contexts with one slug) before the fix.
- Code review (`high`, a9d06bd..b28865d): 7 findings, all fixed (slug collision, reason fallback, reasons lost on
  error, CSP-2 wording, CSP-5 scenario, unpinned test, comment wrap).
