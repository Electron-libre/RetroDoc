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
imports and its `artifact.rs` wraps the saves in `PipelineError`, so the passes did not change. The loaders of the
features, use cases and rules and of the items set aside, the computations made at read time (the reconciliation, the debt report, the domain
models, BM25) and the run history are still in the pipeline: issues of their own move them.
