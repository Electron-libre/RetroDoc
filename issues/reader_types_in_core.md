---
priority: low
depends_on: [one_loader_of_the_generated_docs]
why: 'needed before the readers can leave the pipeline crate, and before technical docs share these objects'
---
# Move the types the readers of the docs consume into retrodoc-core

# Goal
The readers of the generated docs take from `retrodoc-pipeline` the types of what they show (glossary, domain map,
reconciliation, behaviour coverage, scope, usage records), so they depend on the whole pipeline to read a few YAML
files. Expected: these types, and their loading from the cache, in `retrodoc-core` or a module the pipeline does
not own, with the pipeline writing them and the readers reading them.

# Findings
* 2026-11-07: `retrodoc-mcp` uses `Glossary`, `DomainMap`, `domain_models`, `BehaviourCoverage`, `Scope`,
  `SourceHashes`, `reconcile::{reconcile, Judgements, Reconciliation, Status, ItemKind, Contradiction}`,
  `usage_log::{RunUsage, ModelRecord, PassRecord}` and `cache::hash_content` from the pipeline.
* 2026-11-07: `GeneratedDocs::reconciled` calls `reconcile` of the pipeline at read time: a computation, not only a
  type.
* `retrodoc-core` is about 1,700 lines, `retrodoc-pipeline` about 36,000.

# Approach
1. Sort the uses into types to move, loaders to move, and computations (`reconcile`, `domain_models`) to either
   save as artifacts at `generate` time or keep in the pipeline.
2. Move the types in a deliverable of its own (a move edited later counts twice), then the loaders.
3. Check the prints of the fingerprints are unchanged on a saved run (no LLM call on rerun).

# Milestone
The readers of the docs out of the pipeline

# Resources
* ADR 0001 (one-way crates), ADR 0033 (reconciliation), ADR 0037.
* `docs/crates/core-ingest.md`, `docs/crates/pipeline.md`, `docs/crates/render-mcp-cli.md`.

# Hints
* Likely over the size limit of `skill:issue-workflow`: cut at step 1 (types, then loaders, then the reconciliation).

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-19

## Understanding
The readers take their types from the pipeline. This issue moves into `retrodoc-core` the cache layout (`Artifact`,
YAML/JSON load and save, `hash_content`, `cache_format`) then the plain data artifacts the readers load, each with its
`load`/`save` (the loaders move with their type, so nothing is moved twice). Out of scope, split into two new issues:
the computations made at read time (`reconcile`, `build_report`, `domain_models`, `Bm25`) and the run history
(`usage_log`, `RunEstimate`, `recorded_progress`, `format_duration`). Test-only uses (`FakeLlm`, `ContextMap`) stay
dev-dependencies.

Sort of the uses of `retrodoc-mcp` and `retrodoc-web`:

| Use | Kind | Where |
|-----|------|-------|
| `Artifact`, `load_yaml`/`save_yaml`, `hash_content`, `cache_format` | layout | deliverable 1 |
| `Glossary` (+ `Entity`, `ModelFile`, `MergedEntity`…), `DomainMap`/`DomainCluster`, `Scope` | type + loader | deliverable 2 |
| `BehaviourCoverage`, `SetAside`, `SourceHashes` (its `capture` stays), `load/save_features`, `_use_cases`, `_rules`, `load_technical_features`, `load_set_aside` | type + loader | deliverable 3 |
| `reconcile` and its types, `build_report`, `DebtReport` and the report types, `domain_models`, `Bm25` | computation | new issue |
| `usage_log` (`RunUsage` embeds `RunEstimate`), `recorded_progress`, `format_duration`, `thousands` | run history | new issue |

## Plan
1. [x] Cache layout in core: `artifact.rs` and `cache.rs` moved to `retrodoc_core::cache` (`Artifact`, YAML/JSON
   load and save, `hash_content`) with a core `CacheError` the `PipelineError` wraps; `Retention` and the
   `PipelineError` save wrappers stay in the pipeline's `artifact.rs`; the users outside the pipeline import from
   core; ADR 0051; verified by the moved tests and `just check`; ~800 lines. (autonomous)
1b. [x] Format version in core: `cache_format.rs` moved to `retrodoc_core::cache::format` (cut from deliverable 1,
   over the size limit at 1,062 lines); verified by its moved tests and `just check`; ~280 lines. (autonomous)
2. [x] Glossary and domain map in core (`retrodoc_core::artifacts`), with their `load`/`save`; `normalize` to
   `retrodoc_core::text`; core gains `schemars` for `Entity`; verified by the moved tests, `just check`; ~1,000 lines. (autonomous)
2b. [x] Scope in core (cut from deliverable 2, over the size limit at 1,064 lines); verified by `just check`; ~110
   lines. (autonomous)
3. [x] Behaviour coverage and source hashes in core: `BehaviourCoverage` (with `FeatureCoverage`, `EntryCount`) and
   `SourceHashes` (type, `load`/`save`, `get`/`put`/`merge`) in `retrodoc_core::artifacts`; the capture of the hashes
   stays in the pipeline as free functions; verified by the moved tests, `just check` and a rerun of `generate` on a
   saved run that calls no LLM; ~450 lines. (autonomous)
4. [ ] Create the issues of the loaders (features, use cases, rules, set-aside), of the computations and of the run
   history, update `ROADMAP.md` and the `depends_on` of `docs_crate_out_of_the_mcp_crate`; docs.

## Decisions
- Mode: autonomous
- Base commit: a1a27870e170ae2be81d1c2559a9f2e04033ece7
- Place: modules of `retrodoc-core`, no new crate.
- Split: this issue covers the cache layout and the data artifacts; the computations and the run history get issues of their own.
- 2026-11-19, at deliverable 3: the issue reached 2,123 lines; `SetAside` and the loaders of features, use cases
  and rules (127 call sites) go to an issue of their own (answer `1 a`).
- Tests of the readers keep `retrodoc-pipeline` as a dev-dependency.

## Deliverable 1
done: `Artifact`, `load_yaml`/`load_json`/`save_yaml`/`save_json` and `hash_content` live in `retrodoc_core::cache`;
the pipeline keeps a thin `artifact.rs` (`Retention`, save wrappers into `PipelineError`, `warn_on_error`);
`retrodoc-mcp`, `retrodoc-web`, `retrodoc-cli` and `retrodoc-bench` import `Artifact` and `hash_content` from core.

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`) |
| lint | pass (clippy pedantic) |
| specs | pass (no change of behaviour) |
| docs | `CLAUDE.md`, `docs/ARCHITECTURE.md`, `docs/crates/core-ingest.md`, `docs/crates/pipeline.md`, ADR 0051 |
| size | 773 lines (the format move cut to 1b) |

red: none (a move: the moved tests in `retrodoc-core` did not compile before it)

Code review: no findings

not verified: nothing

decisions needed: none

## Deliverable 1b
done: `cache_format` lives in `retrodoc_core::cache::format`; `generate`, the docs readers and `retrodoc-mcp` import
it from core; the round-trip sample test stays in the pipeline (it saves through the pipeline's loaders).

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (no change of behaviour) |
| docs | `docs/crates/core-ingest.md`, `docs/crates/pipeline.md`, `docs/crates/render-mcp-cli.md` |
| size | 295 lines |

red: none (a move)

Code review: no findings

not verified: nothing

decisions needed: none

## Deliverable 2
done: `Glossary` (with `Entity`, `Association`, `ModelFile`, `TestVocabulary`, `MergedEntity`) and `DomainMap`
(`DomainCluster`, `UNCATEGORIZED_SLUG`) live in `retrodoc_core::artifacts`, `normalize` in `retrodoc_core::text`;
`ModelFile::read` became the pipeline's `glossary::read_model_file` (it merges with `merge_chunk_entities`); the
pipeline uses them through `pub(crate)` imports, the other crates import them from core.

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (no change of behaviour) |
| docs | `docs/crates/core-ingest.md`, `docs/crates/pipeline.md` |
| size | 996 lines (`Scope` cut to 2b) |

red: none (a move)

Code review: no findings

not verified: nothing

decisions needed: none

## Deliverable 2b
done: `Scope` (with `load`, `save`, `clear`) lives in `retrodoc_core::artifacts`; the budget (`rank_files`,
`apply_budget`) stays in the pipeline.

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (no change of behaviour) |
| docs | `docs/crates/core-ingest.md` |
| size | 89 lines |

red: none (a move)

Code review: no findings

not verified: nothing

decisions needed: none

## Deliverable 3
done: `BehaviourCoverage` (with `FeatureCoverage`, `EntryCount`) and `SourceHashes` live in
`retrodoc_core::artifacts`; `SourceHashes::capture` and `capture_paths` became the pipeline's `hash_sources` and
`hash_paths` (they read the files with the pipeline's lossy reader), `SourceHashes` gained `FromIterator`.

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (no change of behaviour) |
| docs | `docs/crates/core-ingest.md`, `docs/crates/pipeline.md` |
| rerun | `generate --no-confidence` on a copy of the saved vingilot run with a bogus key: no LLM call, every saved file of identical content (`format.json` added); ledger line |
| size | ~430 lines |

red: none (a move)

Code review: no findings

not verified: nothing

decisions needed: none
