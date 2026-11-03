---
priority: medium
why: 'the bounded contexts of a large application change from one map to another, so a run by context cannot be resumed on a new map; only matters on large applications'
---
# Make the map of bounded contexts steady between runs

# Goal
`retrodoc contexts` maps a large application into bounded contexts with one LLM call; three maps of the same roles
gave 34, 40 and 44 contexts with few names in common, and a share of the names do not fit their files. A user who
redoes the map loses the progress of the contexts already documented, and a technical or catch-all context gives
technical docs. Expected: two maps of the same inputs share most of their contexts and files, and the names fit.

# Findings (large private Rails application, hosted `deepseek-chat`, 2026-10-30, `issues/done/contexts_residual_size_and_names.md`)
* Three maps of the same roles: 34, 40, 44 contexts; 30%, 10%, 10% of the files in the support context; 57% to 81% of
  the files in the same context from one map to another; 4 slugs in all three.
* About 30 of the 44 names fit their files; 5 technical contexts (UI components, configuration, front-end code, base
  classes), 3 catch-alls, 6 one-file contexts.
* The context over the size limit depends on the roles the map starts from (none with local roles, one of 180 files
  with hosted ones).
* `contexts.yaml` is editable and kept by later runs: once written, a map stays unless `--force`.

# Approach
1. Measure first: the share of files that keep their context across the 3 saved maps, by role and by directory, to
   see whether the spread comes from the families, the residue or the names.
2. Candidates (can be combined): (a) seed the map with the deterministic families and let the LLM only name and merge
   them; (b) technical roles (components, config, base classes) go to the support context before the call, never to
   a business one; (c) a one-file context is merged into the context of its closest family; (d) a `--force` map
   keeps the slugs of the previous map when most of their files match.
3. Measure each on the saved roles: 3 maps a setting, about $0.02 each.

# Milestone
Measured quality at the current code

# Resources
* `crates/retrodoc-pipeline/src/contexts/`, ADR 0029, `docs/findings.md` §8, `specs/context-split.md`.
* `scratch/contexts_residual/` beside the saved runs (outside git): the dump and spread scripts.

# Hints
* Never name the application in the repository: "a large Rails application" with figures only.
* Paid runs: ask the user first.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
A `--force` map must keep the contexts already documented and stay close to the previous one, with no technical
context. Out of scope: names that fit (a model's answer) and seeding the map with merged families (candidate (a)),
unless the measure after deliverable 3 still shows the merges spread. Means: spec `specs/context-map.md` (CTX).

Measure of step 1 (no LLM, the 3 saved maps, `scratch/contexts_residual/spread_by.py`, `groups.tsv`):
* Files of a mechanical group follow their context in 72% of the pairs of maps, orphan files (folders) in 57%.
* The merges spread most: of the pairs of groups put together in one map, 11% to 20% are together in the other.
* The support moves: `infra` files 96%, 1%, 0% in support (migrations and initializers given to business contexts
  in maps 2 and 3), `app/components` 85%, 2%, 2%.
* The `contexts` pass is the one pass left at the server's temperature (ADR 0031).

## Plan
1. [x] Temperature 0 for `contexts`: measure 3 maps with `[llm.passes.contexts] temperature = 0` (~$0.05), then add
   `contexts` to the passes at temperature 0 if the spread drops; verified by a test on the request sent; makes true CTX-1. (autonomous)
2. [x] Technical files to the support before the call (roles `infra`, `config`); verified by a test with a fake LLM
   on the listing and the map; makes true CTX-2. (autonomous)
3. [x] `--force` keeps the slug and name of a saved context its files mostly match, so its done mark carries over;
   verified by tests with a fake LLM (match, split, done kept); makes true CTX-3. (autonomous)
4. [x] One-file contexts merged into the context of their folder, else the support; verified by a test; makes true CTX-4. (autonomous)
5. [x] Measure: 3 maps at the final code (~$0.05), then `docs/findings.md` §8, ADR 0029/0031 notes, the ledger;
   sets the spec `accepted`. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 63320ba
- View layers stay out of the early support: only roles `infra` and `config` (question 1).
- Paid measures approved: 3 maps at deliverable 1 and 3 at deliverable 5, ~$0.10 in all (question 2).
- CTX-3 keeps the saved slug and name (question 3).

## Deliverable 1
Measure (3 maps at temperature 0, `contexts_residual-70546ad-2`, `contexts-t0-{1,2,3}.yaml`, `dump-t0-*.tsv`, 15 calls,
51.7k in / 10.7k out, ~$0.02): 49, 42, 38 contexts; support 4%, 24%, 23%; files keeping their context 53% to 80%
(mean 70% against 68% at the server's default); pairs of merged groups kept together 17% to 38% (mean 26% against
16%); 4 slugs in all three; `infra` files in support 0%, 98%, 1%. No clear drop, no harm: `contexts` joins the passes
at temperature 0 (ADR 0031 said so once measured), the support of the technical files is left to deliverable 2.

done: `contexts` asked at temperature 0 unless set (map and split calls).

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass, CTX draft |
| docs | README, `docs/crates/llm.md`, `docs/crates/pipeline.md`, ADR 0031 |
| code review | see below |

red: CTX-1: `left: None, right: Some(0.0)`

Code review: no findings

not verified: the request body sent at 0 (the config test proves the pass setting the provider reads).

decisions needed: none

## Deliverable 2
done: `mapped_sources` leaves the `infra` and `config` files out of the map (`contexts` and `generate`); `contexts`
lists them in the support. A context documented before loses its technical files, so it is pending once.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass, CTX draft |
| docs | `docs/crates/pipeline.md`, ADR 0029, module doc |
| code review | see below |

red: CTX-2: the prompt still listed `db/migrate/ | 1` (the stub kept every source)

Code review: no findings

not verified: the effect on a real map (measured at deliverable 5).

decisions needed: none

## Deliverable 3
done: a `--force` map gives a new context the slug and name of the saved context it shares more than half of the
files with, both ways (one to one); a context whose own slug a kept one took gets `-2`; a done context whose files
did not change stays done. Scenario of CTX-3 amended: a done context stays done only with the same files (its
hash is over its files).

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass, CTX draft |
| docs | `docs/crates/pipeline.md`, `docs/ARCHITECTURE.md`, module doc |
| code review | see below |

red: CTX-3: `left: [("agreements", "Agreements"), ("contracts", "Company contracts")]`; done: `left: []`

Code review: no findings

not verified: a real `--force` map over a saved one (measured at deliverable 5).

decisions needed: none

## Deliverable 4
done: once the map is split, a context of one file moves its groups and folders to the context holding the most
files of its folder (first on a tie), else to the support. Three tests expected the left out one-file group `base`
as a context: their expectations now put it in the support, what they check is unchanged.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass, CTX draft |
| docs | `docs/crates/pipeline.md`, module doc |
| code review | see below |

red: CTX-4: `left: ["contracts", "widgets", "api"], right: ["contracts"]`

Code review: no findings

not verified: the effect on a real map (deliverable 5).

decisions needed: none

## Deliverable 5
Measure (3 forced maps chained from the saved map, `contexts-f778b9b-{1,2,3}.yaml`, 11 calls, 45.9k in / 8.0k out,
~$0.02):

| figure | before (2026-10-30) | temperature 0 only | issue code |
|---|---|---|---|
| contexts | 34 / 40 / 44 | 49 / 42 / 38 | 37 / 30 / 25 |
| largest context | 180 / 143 / 146 | 159 / 140 / 154 | 118 / 150 / 144 |
| one-file contexts | 0 / 5 / 6 | 2 / 3 / 0 | 0 / 0 / 0 |
| support | 29 / 9 / 9% | 4 / 24 / 23% | 26 / 21 / 34% |
| `infra` files in support | 96 / 1 / 0% | 0 / 98 / 1% | 100 / 100 / 100% |
| files keeping their context (mean) | 68% | 70% | 71% |
| merged group pairs kept together | 11-20% | 17-38% | 23-32% |
| slugs in all three maps | 4 | 4 | 12 |

Left (not in the requirements, a follow-up if wanted): the support still moves (UI components 25% to 63%), and
catch-all or layer contexts remain (a UI components context of ~100 files, two catch-alls, an API, a PDF and a
workflows context).

done: findings §8, ADR 0029, README, ledger; spec CTX `accepted` (CTX-1 to CTX-4 traced); `update-docs` over the issue.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass, CTX accepted |
| docs | `docs/findings.md`, ADR 0029, README |
| code review | see below |

red: none

Code review (`high` over 63320ba..HEAD): 10 findings
1. A one-file context joins the context of its layer folder, maybe unrelated: not fixed, the rule validated in CTX-4;
   its behaviours stay documented, under that context.
2. A one-file entry point context alone in its folder went to the support, its behaviours undocumented: fixed, it
   keeps its context (CTX-4 amended, test).
3. The map saved before the split kept its one-file contexts: fixed, merged before each save.
4. A map of one-file contexts only was emptied, then asked again on every run: fixed, kept as it is (test, red
   without the guard).
5. A merge could push a context over 150 files after the size check: fixed, a context of 150 files or more takes
   no merge (test).
6. A kept name next to the new description: not fixed, the description is the answer's for the files the context
   holds now.
7. README said a done context stays done with the same files; the files read for it count too: fixed.
8. The saved map resolved twice: fixed, once.
9. The CLI rebuilds the technical complement: not fixed, display only, the filter has one definition.
10. Slugs of the private application quoted in this issue: fixed, described generically (here and in the findings).

The measure above is at f778b9b, before fixes 2 to 5: a one-file entry point context alone in its folder now
stays, so a later map may hold a few one-file contexts.

Code review (low, on the fixes): no findings; noted: several one-file contexts merged into one target can still take it past 150 files (sizes read before the merges).

not verified: a forced map at the final code (the measure is at f778b9b).

decisions needed: none
