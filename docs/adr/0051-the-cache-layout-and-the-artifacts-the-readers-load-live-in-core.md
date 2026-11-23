# 0051. The cache layout and the artifacts the readers load live in core

Status: Accepted

## Context

The readers of the generated docs (`retrodoc-mcp`, `retrodoc-web`, and through them `ask` and `ui`) take from
`retrodoc-pipeline` the types of what they show (glossary, domain map, scope, behaviour coverage, set-aside items,
source hashes), the functions that load them, the names of the files of `.retrodoc/cache/` and the format version
(ADR 0050). They depend on the whole pipeline, about 36,000 lines, to read a few YAML files, and a reader crate of its
own (issue `docs_crate_out_of_the_mcp_crate`) cannot exist while they do.

## Decision

`retrodoc-core` holds the layout of the cache (`Artifact`, the YAML and JSON load and save, `hash_content`, the format
version) and the plain data artifacts the readers load, each type with its `load` and `save`. The pipeline writes them
and the readers read them, both through core; what computes an artifact from the sources or the LLM (`SourceHashes`
capture, the passes) stays in the pipeline.

Rejected: a new crate for the artifacts (one more crate for a few hundred lines that core, already the shared model,
can hold); moving the types without their loaders (the loaders would be moved a second time).

## Consequences

Core gains `serde_yaml`, `sha2` and `schemars`. Inside the pipeline the moved types are reached through `pub(crate)`
imports and its `artifact.rs` wraps the saves in `PipelineError`, so the passes did not change. The loaders and
savers of the features, use cases and rules and the items set aside followed (`issues/done/loaders_of_the_items_in_core.md`):
their savers return `CacheError`, which the passes convert with `?`. The computations made at read time followed
(ADR 0053), then the run history (`issues/done/run_history_out_of_the_pipeline.md`): the records of `usage.json` with
the saved shape of the estimate, while the pipeline keeps what builds them (`run_usage`, `estimate`); and the
progress of the contexts, which core reads through views of `contexts.yaml` and `fingerprints.json` of its own
rather than take the whole map and the fingerprints, a pipeline test keeping the views true to what it saves.
`retrodoc-mcp` then depends on the pipeline only for its tests.
