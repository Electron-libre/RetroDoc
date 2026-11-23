---
priority: low
why: 'a measure with no LLM that no milestone needs; it decides whether resolved references deserve work at all'
---
# Measure whether the references codegraph resolves beat the lexical slices and the entry points

# Goal
RetroDoc links files by name: slices follow referenced identifiers as text, `ranking.rs` counts the files mentioning
a file's name, contexts and routes are placed by the names their files write. A call resolved by its name alone was
measured as noise (`issues/done/parsed_references_for_ranking_and_slices.md`). codegraph (MIT, open source) claims
to resolve calls, imports and inheritance across files to their definition, and to read the routes of 20+ web
frameworks with no LLM. Expected: a measure, with no LLM and no paid call, of whether its references and routes
would change RetroDoc's slices and entry points for the better; then a decision to drop, plug in or port it.

# Findings
* Measured 2026-10-28 (`parsed_references_for_ranking_and_slices`, deliverable 1): slices from calls resolved by
  name differ from the lexical ones by as much noise as they remove; the noise comes from method names called on a
  receiver whose type the parser does not know, resolved to any file defining a method of that name. Resolving only
  to classes, modules and functions makes the slices almost the lexical ones. The `parsed_slices` example and
  `Follow` stayed as the measuring tool.
* Read from the codegraph repository on 2026-11-23 (its README, `package.json`, `codegraph-kernel/Cargo.toml`,
  `src/lib.rs`, `src/db/schema.sql` and the release page, nothing run): v1.6.4 of 2026-11-20.
  * Its Rust kernel (`codegraph-kernel`, `publish = false`) is a Node addon (`cdylib`, napi exports only) that only
    parses and extracts each file, which `structure.rs` already does. The resolution across files, the framework
    routes and the SQLite store are in TypeScript.
  * Ways in for a Rust program: the CLI as a subprocess (`--json` on `query`, `callers`, `callees`, `impact`,
    `files`, `status`; `context --format json`; not confirmed for `explore`), its SQLite file
    `.codegraph/codegraph.db` (tables `nodes` and `edges`, edge kinds calls, imports, extends, implements,
    references; versioned by migrations, not stated stable), or its MCP server (`codegraph serve --mcp`).
  * The release archives (about 60 MB each) bundle their own Node runtime; the npm package needs Node 20 or later.
  * Anonymous telemetry is on by default (`CODEGRAPH_TELEMETRY=0` or `DO_NOT_TRACK=1` turns it off). Its figures
    (cross-file coverage 74 to 100% per language, fewer tokens for coding agents) are its own, not checked here.

# Approach
Measure first, with no LLM, on the frozen inputs of the benchmark repositories (linkding, Lobsters) and the default
smoke repository; telemetry off; the confidential repositories only if the user agrees.

1. Index each repository with the codegraph CLI and export its edges (CLI `--json`, else a read of its SQLite file
   by a throwaway script, not by RetroDoc).
2. Slices: add a fourth slice to the comparison of `parsed_slices` (lexical, mixed, parsed alone, codegraph), per
   entry-point file of the saved runs, same bounds (depth 2, 8 files): files in common and on one side, read on a
   sample as the 2026-10-28 measure was, method calls first, where the name resolution failed.
3. Entry points: compare the routes codegraph finds with the `entry-points.yaml` of the saved runs (found by both,
   by one side only).
4. On a gain a reader can see, propose, for the user to choose: (a) an optional accelerator that calls the codegraph
   CLI when it is installed, with today's path as the fallback (ADR 0036); (b) a port to Rust of its resolution
   (import-aware, receiver types) into `structure.rs`, keeping one binary with no Node and no telemetry; (c) codegraph
   as a tool of the `ask` and `chat` agent only (callers, callees, impact). With no gain, record why and close.

# Resources
* `issues/done/parsed_references_for_ranking_and_slices.md` (its results and decisions)
* ADR `0022`, ADR `0027`, ADR `0036`
* `crates/retrodoc-ingest/src/structure.rs`, `crates/retrodoc-pipeline/src/slices.rs`, `ranking.rs`
* the `parsed_slices` example of `retrodoc-bench` (`docs/dev/commands.md`)
* `docs/crates/pipeline.md` (`contexts/`, `route_owner.rs`)
* the codegraph repository: `https://github.com/colbymchenry/codegraph`

# Hints
* The ledger of measures lists the saved runs and frozen inputs to reuse; append the measure there.
* Never let codegraph's telemetry run on a confidential repository.
* Its schema is internal: a measure may read it, RetroDoc code must not depend on it.
