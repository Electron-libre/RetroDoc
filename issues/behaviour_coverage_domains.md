# Build domains and features from entry points and entities, and cover behaviours instead of files

# Goal

Make the cost of a run grow with the business surface of the application instead of its file count, and
stop forcing technical files into business domains. Implement ADR 0020: every entry point belongs to one
use case of one feature, domains are clustered from the entities and the entry points, features group
entry points, the LLM repo map becomes optional, and files reached by no behaviour are reported as
technical support.

# Findings

* First full `generate` on the Rails test repository (2026-10-02, 325 source files, local model): about
  2h20, with 201 file calls and 89 directory calls for the repo map; the ~2,300-file repository is still
  out of reach (`PLAN.md` §7.2).
* The features pass reads only the file summaries of its domain (`features/mod.rs`), not its entry points
  nor the glossary.
* The same file is read by the LLM in the glossary, entry points, repo map, use cases and confidence
  passes.
* The confidence pass is about a third of the calls (`PLAN.md` §7.2).

# Approach

1. Measure first on `delivery_router` and a sparse checkout of the Rails test repository: share of files
   reached by an entry point slice or holding an entity, calls and time per pass (`usage.json`).
2. Domains from the surface: cluster entities and entry points grouped by resource, framed by the product
   brief (`issues/done/product_brief.md`); attach files deterministically from slices and entity homes.
3. Features from entry points: one call per domain over its entry points (verb, resource, outputs) and
   entities; use cases keep their per-entry-point slices.
4. Coverage of entry points by construction (place, then uncategorized), reported; files reached by
   nothing are listed in the debt report.
5. Repo map: make the LLM summaries opt-in; use a deterministic map (symbols, references, ranking) for
   the structure, see `issues/tree_sitter_structure.md`.
6. One read per business file: one call returns the entities, entry points, outputs and rules of a file.
7. Confidence: deterministic checks first (named identifiers at the cited lines, named entities in the
   slice), the LLM verdict on a sample and on the doubtful steps.
8. Rekey the incremental re-run on entry points and entities.
9. Compare with `issues/quality_benchmark.md`.

# Resources

* ADR `0020`, and the ADRs it affects: `0004`, `0005`, `0008`, `0011`
* `crates/retrodoc-pipeline/src/domains/`, `features/`, `use_cases/`, `slices.rs`, `ranking.rs`,
  `repo_map/`, `confidence/`, `fingerprints.rs`
* `crates/retrodoc-cli/src/commands/generate/mod.rs` (pass order)

# Hints

* This is the largest change of the redesign: ship it in steps that each leave `generate` working, and
  keep the old path behind a setting until the benchmark says the new one is better.
* A library's public API is its entry points (ADR 0008); `delivery_router` is a library.
* A missed entry point is a missed use case: the report must make the gap visible.

# Tracking

Each deliverable keeps `generate` working: the new path is behind `[pipeline] domains_from = "surface"`, the
default stays `files` (the old path) until deliverable 6.

1. [x] Measure the old path (step 1): an example that reads the saved caches and gives, per repository, the share
   of source files reached by an entry point slice or holding an entity, and the calls and time per pass
   (repo map and domains included). Table in this issue. No LLM call.
2. [x] The setting and the domains from the surface (step 2; done, waiting for the user's review): `[pipeline] domains_from` in `retrodoc-core`; in
   `surface` mode `generate` builds no LLM repo map; one call groups the resources (entry points by resource)
   and the entities into domains, framed by the brief; files are attached mechanically. The unit assigned is
   the resource, not the file.
3. [x] Features from the entry points (step 3): one call per domain over its entry points and entities;
   `Feature` gets an `entry_points` field (absent from old `features.yaml`); the use cases pass reads those
   entry points when present, so an entry point has one use case.
4. [x] Coverage of the entry points by construction (step 4): unplaced ones go through a small call, then an
   uncategorized bucket; unknown names dropped, first assignment wins; the files reached by no behaviour are
   listed as technical support in the debt report and in `_retrodoc/coverage-report.md`.
5. [ ] Incremental re-run (step 8): domains and features fingerprints rest on the entry points and entities of
   the unit; an unchanged run makes no call.
6. [ ] Compare and decide (step 9): quality benchmark and calls/time per pass, `files` against `surface`, on
   linkding and `delivery_router`; decision on the default; ADR 0020, `PLAN.md`, `CLAUDE.md`; follow-up issues
   for the deterministic repo map, the single read per file and the deterministic confidence.

## Decisions

* Cut: steps 1 to 4, 8 and 9 in this issue; steps 5 (deterministic map), 6 (single read per file) and 7
  (deterministic confidence) go to separate issues, queued at deliverable 6.
* Setting `[pipeline] domains_from = "files" | "surface"`, default `files` until deliverable 6.
* In `surface` mode the LLM repo map is not built (no pass of the new path reads it).
* Entry points are assigned by resource, not by file: a file can carry several domains.
* Domains are flat in `surface` mode. Entry points are cited by id `<file>#<name>`; a feature of that mode
  lists them in `Feature.entry_points` and its `source_paths` are the files of those entry points only (no
  entity files). The use cases pass reads exactly those entry points. Entry points no feature claimed are
  only logged until deliverable 4.
* Entry points no feature holds go through one placement call per batch of 60 (existing features only), then
  stay in `behaviour-coverage.yaml` (`unplaced_entry_points`) and the report; they get no use case. The files
  reached by no behaviour (`technical_support`) are computed by `generate` after the features. Both are
  written only by a `surface` run.
* No paid run without asking.
* Follow-up to queue at deliverable 6: the time of a run is in the use cases and confidence passes (73 + 17 of
  118 minutes locally), not in the repo map; an issue on the cost per use case (batching, fewer or shorter
  calls) is needed for the new path to cut the time, not only the calls.

## Baseline of the old path (2026-10-09, saved runs, no LLM call)

`cargo run -q -p retrodoc-pipeline --example behaviour_baseline -- <run dir>...` on the benchmark runs. "Reached"
= source files holding an entry point, mentioned by an entity, or in the slice (depth 2, 8 files, as the use
cases pass) of the file of an entry point. Calls and minutes are those of the last `generate` of each run.

| Repository / model | Runs | Source files | Reached | Calls | Minutes | Repo map calls (minutes) |
|---|---|---|---|---|---|---|
| `delivery_router`, deepseek-chat | 10 | 9 | 44-100% (9 runs at 100%) | 16-26 | 0.6-1.0 | 5 (0.1) |
| `delivery_router`, local qwen3.6 | 10 | 9 | 44-100% (4 runs at 100%) | 21-27 | 3.5-11 | 5-6 (0.3-0.4) |
| linkding, deepseek-chat | 4 | 178-188 | 37-53% | 222-283 | 9.7-14.4 | 135-138 (2.3-2.5) |
| linkding, local qwen3.6 | 3 | 176-177 | 44-48% | 260-320 | 86-118 | 133-134 (10-10.7) |
| a Rails application (private), cache of an older run | 1 | 1,661 | 36% | - | - | - |

Per pass of two linkding runs (calls / minutes / tokens in):

| Pass | Local qwen3.6 | deepseek-chat |
|---|---|---|
| entry-points | 13 / 10.4 / 33k | 12 / 0.7 / 31k |
| repo-map | 133 / 10.7 / 115k | 138 / 2.4 / 114k |
| domains | 1 / 0.8 / 3k | 1 / 0.1 / 3k |
| features | 14 / 4.1 / 11k | 13 / 0.6 / 11k |
| use-cases | 80 / 72.6 / 265k | 111 / 10.4 / 397k |
| confidence | 72 / 17.2 / 160k | (not run) |
| total | 320 / 118.4 | 283 / 14.4 |

* On a web application about half of the source files are reached by no entry point nor entity (36% to 53%
  reached): they are the "technical support" the new invariant would leave out of the docs.
* The repo map is 45% to 55% of the calls of linkding but only 9% (local) to 17% (deepseek) of the time: its
  prompts are small. Skipping it saves calls and about 115k input tokens, not the hours. The time is in the
  use cases (73 of 118 minutes locally, 10 of 14 on deepseek) and the confidence pass (17 minutes), which grow
  with the number of features and entry points, not with the files.
* On `delivery_router` the share reached swings between 44% and 100% from run to run, with the entry points
  found (see `issues/done/business_files_for_entry_points_and_actors.md`): the quality of the entry points
  pass bounds what the new path can document.
* Limits: the reach counts the files of one slice per entry file (the use cases cut it per feature); two
  runs only for the per-pass table; the Rails row is an older cache with no usage log.

Consequence for the next deliverables: the cost gain to expect from skipping the repo map is in calls and
tokens; a gain in time needs the use cases and confidence passes to cost less, which the new path does not
change by itself. The comparison of deliverable 6 should report both.

## Domains from the surface, on real data (2026-10-09, local `qwen3.6:35b-a3b`, `generate --force`, `domains_from = "surface"`)

* `generate` stops after the domains as planned, with no repo map: 8 calls on `delivery_router` and 37 on
  linkding, none for the repo map (14 min on linkding, 8 of them for the entry points, 3 for the glossary).
* First prompt: the model cited verbs and method names as resources (`add_order`, `import`, `manage`,
  `view`...), 35 of linkding's 36 resources fell into "uncategorized" and 4 of `delivery_router`'s 2 were
  invented. The listing showed each resource with its verbs; the model copied the verbs.
* Prompt tightened (names quoted, "never an action, a method or an endpoint"), same inputs, domains redone:
  `delivery_router` 2 domains (order management, route assignment) with its 2 resources and 4 entities placed;
  linkding 5 domains (bookmarks, organization and classification, user profile, system configuration and data,
  API access) with 25 of 36 resources and 16 of 17 entities placed, 11 resources and 1 entity left in
  "uncategorized" (deliverable 4 places them with a small call).
* One run per repository and prompt: these figures say it works, not how stable it is.
* The prompt text is not part of the domains fingerprint (as everywhere): after changing it the saved
  `domains.yaml` has to be removed.
