---
priority: low
why: 'only large applications mapped into contexts; the progress of the runs already survives a new map, what is left is the share of the files that moves and contexts that read as technical'
---
# Make the support of the context map steady and drop its catch-all contexts

# Goal
After `issues/done/context_map_spread_between_runs.md`, a forced map keeps the slugs of the contexts it matches and
leaves the technical files out, but the support still moves from one map to another and some contexts are a layer
or a catch-all, which gives technical docs. Expected: two maps of the same inputs put about the same files in the
support, and no context is named after a layer or holds unrelated leftovers.

# Findings (large private Rails application, hosted `deepseek-chat`, 2026-11-04, 3 forced maps at f778b9b)
* Support 26%, 21%, 34% of the files; UI components (role `view`) 25%, 30%, 63% in the support, models 27%, 15%, 40%.
* Files keeping their context 57% to 78% (mean 71%, 68% before the issue); of the pairs of groups merged in one
  map, 23% to 32% are merged in another: the merges still depend on the answer.
* Layer or catch-all contexts in each map: a UI components context of ~100 files, one or two catch-alls (~70 and
  ~16 files), an API, a PDF and a workflows context.
* Temperature 0 alone changed little (`docs/findings.md` §8).

# Approach
1. Measure first, no LLM: on the saved maps and dumps, which folders and groups move in and out of the support,
   and what the catch-all and layer contexts are made of (orphan folders, small groups).
2. Candidates (can be combined): (a) seed the map with deterministic families (groups sharing a name across
   folders, already found for the split) and let the LLM only name and merge them; (b) give each orphan view or
   model file to the context of the group its name or its references reach, before the call; (c) a context named
   after a layer or a kind of folder, or a catch-all, is asked again or spread over the contexts its files
   reference; (d) list the support of the saved map to a forced call as a proposal.
3. Measure each on the saved roles: 3 forced maps a setting, about $0.02 each.

# Specs
* `specs/context-map.md` (CTX): new requirements for the support and the layer contexts.

# Milestone
Measured quality at the current code

# Resources
* `crates/retrodoc-pipeline/src/contexts/`, ADR 0029, `docs/findings.md` §8, `specs/context-map.md`,
  `issues/done/context_map_spread_between_runs.md`.
* The spread scripts and dumps beside the saved runs (outside git, see the measures ledger).

# Hints
* Never name the application in the repository, nor quote its context slugs: "a large Rails application" with
  figures only.
* Paid runs: ask the user first.
* A one-file entry point context alone in its folder is kept on purpose (CTX-4).

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
Make the support of a forced map steady and remove the layer and catch-all contexts, with deterministic rules applied
after the call (the map keeps folder and group rules, not files, so orphan files are placed folder by folder by the
names their files are written under, not one by one before the call). Out of scope: the roles pass that gives the
migrations the role `model`, the groups named after a namespace word (`api`, `admin`).

## Measure without LLM (step 1, the 3 maps at f778b9b)
* 538 of 1,676 files move in or out of the support between maps: half are 38 whole groups the answer sets aside in
  one map and not in another (all hold an entry point file; two of 30 files in one map are core business), half are
  orphan folders (UI components 82 of 100, top-level models, two `lib/` libraries, 45 migrations whose role is
  `model`, so CTX-2 keeps them in the map).
* Each layer context is made of orphan folders: the UI components context holds 78 to 88 folder files of ~95, the
  models, mail senders, PDF library, workflow actions and validation strategies contexts are folder files only.
* The catch-alls are many small groups (15 groups and 71 files; 9 groups and 16 files).
* Orphan files whose class name the source files of a group write: 139 of 495 to one group first, 70 tied, 286 to
  none (76 of the 78 UI components moving, all migrations).

## Plan
1. [x] Layer contexts spread: a context mostly made of folder files lists each folder file in the context whose files
   write its class name the most, else the support for a view or a model, else keeps it; verified by tests on maps
   built in memory; makes true CTX-5. (autonomous)
2. [x] Catch-all contexts spread: same placement for each group and folder file of a context with a catch-all slug,
   the prompt forbids such names; verified by tests; makes true CTX-6. (autonomous)
3. [x] Saved support proposed: a forced call lists the saved support as the proposed support; verified by a
   test on the request sent; makes true CTX-7. (autonomous)
4. [ ] Measure and docs: deliverables 1 and 2 replayed on the 3 saved maps without LLM, then 3 forced maps chained
   from the saved one (~$0.06, asked first); findings §8, ADR 0029, spec `accepted`, ledger; verified by the figures
   against the f778b9b baseline.

## Decisions
- Mode: autonomous
- Base commit: e369bae8ae7c3c7e4ac588744a08e1f9401bef36
- Support rule (CTX-7): the saved support is proposed to the forced call, not kept mechanically.
- Measure: deliverables 1 and 2 replayed without LLM on the saved maps, then one series of 3 forced maps after deliverable 3 (~$0.06).
- Layer contexts (question 1 at the first deliverable, 2026-11-04): the folder files are placed one by one, listed in
  `contexts.yaml` (`files`, ADR 0029 amended): a file goes to the other context whose files write its class name
  the most, else to the support when it is a view or a model, else stays. Moving a folder whole was measured on the
  3 saved maps: the top-level models folder went whole to one context, and ~30 of the 34 workflow tasks, a business
  family written by its own context, went to the support.

## Deliverable 1
done: a layer context lists each of its folder files in the context that writes its class name the most, else in
the support for a view or a model; `contexts.yaml` gains `files` per context and support; the map call takes the
roles and reads the sources.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass, CTX draft (CTX-5 traced) |
| docs | module doc, `docs/crates/pipeline.md`, ADR 0029, `docs/ARCHITECTURE.md` |
| code review | see below |

red: CTX-5: `assertion left == right failed: the group files stay, left: 7, right: 3`

Code review: no findings

not verified: the effect on the real maps (deliverable 4).

decisions needed: none

## Deliverable 2
done: a context whose slug is made of catch-all words only gives each group to the context that writes the class
names of its files the most, else to a context of its own, and each folder file likewise, else to the support; the
prompt forbids catch-all names. Two split tests named a part `other`: renamed `partners`, what they check unchanged.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass, CTX draft (CTX-6 traced) |
| docs | `docs/crates/pipeline.md`, ADR 0029, module doc |
| code review | see below |

red: CTX-6: `left: ["billing", "misc-business"], right: ["billing", "user"]`

Code review: no findings

not verified: the effect on the real maps (deliverable 4).

decisions needed: none

## Deliverable 3
done: the call of a forced map lists the groups and folders of the saved support that it shows, as the support
proposed; the split call proposes none.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass, CTX draft (CTX-7 traced) |
| docs | `docs/crates/pipeline.md`, ADR 0029 |
| code review | see below |

red: CTX-7: `prompt has no line "Support of the previous map, proposed: groups g3; folders db/migrate/"`

Code review: no findings

not verified: the effect on the real maps (deliverable 4).

decisions needed: none
