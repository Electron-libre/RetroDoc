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
