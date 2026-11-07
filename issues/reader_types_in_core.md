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
