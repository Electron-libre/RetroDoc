---
priority: low
depends_on: [one_loader_of_the_generated_docs, format_version_of_the_saved_artifacts, reader_types_in_core]
why: 'no milestone needs it yet; it is the ground of any export, hosted or third-party reader, so before those'
---
# Read the generated docs through a crate of their own, the MCP crate left as a server

# Goal
`retrodoc-mcp` holds the read model of the generated docs, the search, the run history, the stores of the reviews
and edits, the audit and the debt view, and also the MCP server; the web interface, the `ask` agent, the benchmark
and the CLI depend on it for the docs, not for MCP. Expected: a `retrodoc-docs` crate (read model, stores, history,
search) that does not depend on the pipeline, read by every reader, with `retrodoc-mcp` left as the MCP server over
it.

# Findings
* 2026-11-07: `retrodoc-mcp` depends on `retrodoc-pipeline`, `retrodoc-ingest` and `retrodoc-core`; its modules are
  `audit`, `catalog`, `corpus`, `debt`, `edits`, `freshness`, `history`, `read_model`, `reviews`, `search`, `server`,
  `source`, `store`, `tools` (about 10,300 lines).
* 2026-11-07: `retrodoc-web`, `retrodoc-agent`, `retrodoc-bench` and `retrodoc-cli` depend on `retrodoc-mcp`.
* The prerequisites are the issues of `depends_on`: one loader, a format version, the consumed types out of the
  pipeline.

# Approach
1. Once the prerequisites are done, list the modules that are about the docs and those about MCP (`server`, the
   wording of the tool answers).
2. Move the docs modules to `retrodoc-docs` in a deliverable of its own, then point each reader at it.
3. An ADR for the split; `docs/ARCHITECTURE.md`, the crate docs and the dependency graph of `CLAUDE.md` updated.

# Milestone
The readers of the docs out of the pipeline

# Resources
* ADR 0001, 0016, 0039, 0043, 0044, 0045, 0047.
* `docs/ARCHITECTURE.md`, `docs/crates/render-mcp-cli.md`.

# Hints
* The `catalog` of read-only tools serves both the MCP server and the `ask` agent: decide where it lives at the plan.
* The rerun must stay a no-op.
