---
priority: low
depends_on: [reader_types_in_core]
why: 'the last loaders the readers take from the pipeline; needed before the docs crate'
---
# Move the loaders of the features, use cases and rules into retrodoc-core

# Goal
The readers of the generated docs still load the features, use cases, rules, the technical features and the items
the rules pass set aside through `retrodoc-pipeline` (`load_features`, `load_use_cases`, `load_rules`,
`load_technical_features`, `load_set_aside`), and their tests save them with `save_features`, `save_use_cases`,
`save_rules`. Expected: these loaders and `SetAside` in `retrodoc_core::artifacts`, next to the artifacts
`reader_types_in_core` moved, the pipeline writing through them.

# Findings
* 2026-11-19: 127 lines outside the pipeline call these functions or name `SourceHashes`/`BehaviourCoverage`, most
  of them `save_features`/`save_use_cases`/`save_rules` in the tests of `retrodoc-web`; that count made
  `reader_types_in_core` cut them out (its deliverable 3).
* `SetAside` (`rules/aside.rs`, with `AsideFeature`, `AsideRule`) has methods the rules pass uses
  (`of_feature`, `forget_features`, `extend`, `pub(super)`): they become public in core or free functions of the
  pipeline.
* The loaders are one line each over `Artifact` and `load_yaml`/`save_yaml`, already in `retrodoc_core::cache`.

# Approach
1. Move `SetAside` and the five loaders and three savers to `retrodoc_core::artifacts`, the savers returning
   `CacheError`.
2. Point every caller at core, the tests of `retrodoc-web` included (a mechanical rewrite of the paths).
3. Check a rerun of a saved run calls no LLM and rewrites no file.

# Milestone
The readers of the docs out of the pipeline

# Resources
* ADR 0051, `issues/done/reader_types_in_core.md` (`# Tracking`: the sort of the uses).
* `crates/retrodoc-core/src/artifacts/`, `docs/crates/core-ingest.md`.

# Hints
* The rewrite of the call sites counts twice in the size of a deliverable (a line removed and a line added): keep
  the moves and the rewrite in separate deliverables if one goes over the limit.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-23

## Understanding
Move `SetAside` (with `AsideFeature`, `AsideRule`) and the loaders and savers of the features, use cases, rules,
technical features and set-aside items from `retrodoc-pipeline` to `retrodoc_core::artifacts`, then point every
caller at core. Pure move, no change of behaviour (no spec). Out of scope: the private loaders of the passes
(`load_stated`, `load_surface_features`, `save_surface_features`), which only the pipeline reads.

## Plan
1. [x] Loaders and `SetAside` in core: `retrodoc_core::artifacts::items` (`load_features`, `save_features`,
   `load_technical_features`, `load_use_cases`, `save_use_cases`, `load_rules`, `save_rules`, the savers returning
   `CacheError`) and `retrodoc_core::artifacts::set_aside` (`SetAside` with `of_feature`, `forget_features`,
   `extend` made public, `load`); the pipeline uses them and re-exports them for now, so no caller outside
   changes; verified by `just check` and a unit test of the round trip in core; ~350 lines. (autonomous)
2. [ ] Callers on core: `retrodoc-web`, `retrodoc-mcp`, `retrodoc-cli`, `retrodoc-bench` import the loaders and
   `SetAside` from core, the pipeline's public re-exports dropped; docs (`docs/crates/core-ingest.md`,
   `docs/crates/pipeline.md`, ADR 0051); verified by `just check` and a rerun of `generate` on a copy of a saved run
   (bogus key) that calls no LLM and rewrites no file; ~350 lines.

## Decisions
- Mode: autonomous
- Base commit: 89786694c2296f0ad50735af8a3580b8c1619b92

## Deliverable 1
done: `SetAside` (with `AsideFeature`, `AsideRule`, now with `load`/`save`, its methods public) lives in
`retrodoc_core::artifacts::set_aside`, the loaders and savers of the features, use cases and rules in
`retrodoc_core::artifacts::items` (the savers return `CacheError`); the pipeline re-exports them for its callers
(`load_set_aside` delegates to `SetAside::load`) and `warn_on_error` takes any displayable error.

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`), 3 new tests in core |
| lint | pass |
| specs | pass (no change of behaviour) |
| docs | `docs/crates/core-ingest.md`, `docs/crates/pipeline.md`, ADR 0051 |
| size | 375 lines |
| file size | `use_cases/tests.rs` 1,882 lines (1,881 before, one import added): not split, out of this move |

red: none (a move: the new core tests cover code moved as is)

Code review: no findings

not verified: nothing

decisions needed: none
