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
