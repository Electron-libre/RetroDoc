---
priority: low
depends_on: [one_loader_of_the_generated_docs, format_version_of_the_saved_artifacts, reader_types_in_core, loaders_of_the_items_in_core, read_time_computations_out_of_the_pipeline, run_history_out_of_the_pipeline]
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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-23

## Understanding
Split `retrodoc-mcp` in two: a new `retrodoc-docs` crate holds every module about the generated docs (read model,
corpus, search, freshness, source access, stores of reviews and edits, debt, audit, history, report, and the
read-only tools with their catalog); `retrodoc-mcp` keeps only `server.rs` (the `rmcp` wiring) over it. Pure
refactor: no change of behaviour, no spec amended, the rerun stays a no-op.

## Plan
1. [x] Prepare the move in place: the argument types of the catalog and `search_limit` public, the fixtures of
   `corpus` public behind a `testing` feature, so the server can reach them from another crate; verified by
   `cardinal run project-check`. (autonomous)
2. [x] Move the docs modules to `retrodoc-docs`: `git mv` of every module of `retrodoc-mcp` but `server.rs` (and of its
   two integration tests) into `crates/retrodoc-docs/`, a new `lib.rs` and manifest, `retrodoc-mcp` reduced to
   `server.rs` over `retrodoc-docs`; the readers (agent, web, bench, cli) pointed at `retrodoc_docs`; ADR draft
   0054; verified by `cardinal run project-check` (the moved tests pass unchanged, the spec traceability holds) and
   `cargo tree` (no reader but the CLI depends on `retrodoc-mcp`, `retrodoc-docs` has no normal dependency on the
   pipeline). (autonomous)
3. [x] Docs: ADR 0054 accepted, `docs/ARCHITECTURE.md` (graph and table), the dependency graph of `CLAUDE.md`,
   `docs/crates/` (a `docs.md` of the new crate, `render-mcp-cli.md` left with the server), the doc comments that
   name `retrodoc-mcp` for the read model; verified by `skill:update-docs` and a grep of `retrodoc-mcp` and
   `retrodoc_mcp` left only where the server is meant. (autonomous)
4. [x] Fixes of the `high` review: the status lines of ADRs 0016, 0039 and 0047 refined by 0054, `report.rs` in
   `docs/crates/docs.md`, the public docs of `source.rs` without links to private constants; verified by
   `RUSTDOCFLAGS=-D warnings cargo doc -p retrodoc-docs` and `cardinal run project-check`. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: d66bfd81099cf58144c9ea6a161ce2a6594c7f2f
- Catalog and tools go to `retrodoc-docs` (answer 1 a).
- `retrodoc-pipeline` stays a dev-dependency of `retrodoc-docs` for the audit tests (answer 2 a).
- The move was cut in two for its size (2,270 lines in one: a moved file that is also edited counts twice): the
  edits of `catalog.rs` and `corpus.rs` first, in place (deliverable 1), then the move alone (deliverable 2).

## Summaries

### Deliverable 1
- done: the catalog's argument types and `search_limit` public, the fixtures of `corpus` public behind the
  `testing` feature of `retrodoc-mcp`; no change of behaviour.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | clippy pedantic clean |
| specs | traceability unchanged |
| docs | none touched (the crate docs follow at deliverable 3) |

- red: none (refactor, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none


### Deliverable 2
- done: `retrodoc-docs` created by moving every module of `retrodoc-mcp` but `server.rs`, and its two integration
  tests, unchanged; `retrodoc-mcp` reduced to the server over it; the agent, web, bench and CLI read the docs through
  `retrodoc_docs`, only the CLI depends on `retrodoc-mcp` (`cargo tree -i`).

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`), the moved tests unchanged |
| lint | clippy pedantic clean |
| specs | traceability unchanged |
| docs | the crate docs, the architecture and the graph of `CLAUDE.md` at deliverable 3 |
| size | 848 changed lines (the moves are pure renames) |

- red: none (refactor, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none

### Deliverable 3
- done: ADR 0054 accepted; `docs/crates/docs.md` holds the detail of the read model moved from
  `render-mcp-cli.md`, which keeps the server; the graphs and tables of `CLAUDE.md`, `docs/ARCHITECTURE.md` and
  `README.md` name eleven crates; the doc comments and crate docs that meant the read model name `retrodoc-docs`.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | clippy pedantic clean |
| specs | traceability unchanged |
| docs | `skill:update-docs` over `d66bfd8..HEAD`; `retrodoc-mcp` left only where the server is meant |

- red: none (docs only)
- Code review: no findings
- not verified: the Mermaid graph of `docs/ARCHITECTURE.md` not rendered
- decisions needed: none

### Deliverable 4
- done: the fixes of the `high` review of `d66bfd8..HEAD` (4 findings, no correctness bug): ADRs 0016, 0039 and
  0047 say what 0054 refines in them; `report.rs` listed in `docs/crates/docs.md` (pre-existing gap);
  `source.rs` documents its private bounds as code spans, so `cargo doc` of the crate is warning-free
  (pre-existing). Not fixed: no commit scope added for `retrodoc-docs` (the `docs` scope, already allowed, would
  read as the documentation one): left to the user.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | clippy pedantic clean; `RUSTDOCFLAGS=-D warnings cargo doc -p retrodoc-docs -p retrodoc-mcp` clean |
| specs | traceability unchanged |
| docs | the ADR status lines, `docs/crates/docs.md` |

- red: none (docs and doc comments)
- Code review: no findings
- not verified: none
- decisions needed: a commit scope for `retrodoc-docs`
