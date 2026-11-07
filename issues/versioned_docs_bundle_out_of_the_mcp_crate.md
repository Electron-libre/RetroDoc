---
priority: low
why: 'no milestone needs it yet; it is the ground of any export, hosted or third-party reader, so before those'
---
# Read the generated docs through a versioned bundle in a crate of their own

# Goal
Every reader of the generated docs (MCP server, search, web interface, `ask` agent, benchmark) loads the artifacts
of `.retrodoc/cache/` through `retrodoc-mcp`, with types of `retrodoc-pipeline`, and nothing says which format a
saved run is in. Expected: a read model in a crate that does not depend on the pipeline, over a documented and
versioned format of the generated docs, with `retrodoc-mcp` left as the MCP server over it.

# Findings
* 2026-11-07: `retrodoc-mcp` depends on `retrodoc-pipeline` and uses its types (`Glossary`, `reconcile::*`,
  `usage_log::*`, `BehaviourCoverage`, `SourceHashes`, `save_features`).
* 2026-11-07: `retrodoc-mcp` holds the read model, the search, the run history, the stores of the reviews and edits,
  the audit, the debt view and the server; `retrodoc-web`, `retrodoc-agent`, `retrodoc-bench` and `retrodoc-cli`
  all depend on it.
* 2026-11-07: no format or schema version is written in the artifacts (no match for a version constant in
  `crates/`).
* `retrodoc-core` is about 1,700 lines against about 36,000 for the pipeline.

# Approach
1. List what the readers take from the pipeline and from the cache (types, files, fields); measure what moving
   them to `retrodoc-core` costs.
2. Candidates, to choose at the plan:
   1. a `retrodoc-docs` crate (read model, stores, history) over `retrodoc-core` only, the pipeline writing the
      bundle and the readers reading it;
   2. the consumed types moved to `retrodoc-core` and the crates left as they are;
   3. a format version written with the artifacts and checked at load, with a clear error on an older run.
3. An ADR for the chosen split; `docs/ARCHITECTURE.md` and the crate docs updated.

# Resources
* ADR 0001, 0003, 0016, 0039, 0043, 0044, 0045, 0047.
* `docs/ARCHITECTURE.md`, `docs/crates/render-mcp-cli.md`, `docs/crates/pipeline.md`.

# Hints
* Large refactoring: likely over the size limit of `skill:issue-workflow`; cut it at step 1 (format version first,
  then the types, then the crate).
* The rerun must stay a no-op: moving a type must not change the prints of the fingerprints.
