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
* ADR 0051, `issues/reader_types_in_core.md` (`# Tracking`: the sort of the uses).
* `crates/retrodoc-core/src/artifacts/`, `docs/crates/core-ingest.md`.

# Hints
* The rewrite of the call sites counts twice in the size of a deliverable (a line removed and a line added): keep
  the moves and the rewrite in separate deliverables if one goes over the limit.
