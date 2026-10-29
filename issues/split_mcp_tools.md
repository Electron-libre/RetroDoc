# Split the MCP tools file into its tests and one file per tool

# Goal
`crates/retrodoc-mcp/src/tools.rs` holds every MCP tool, their shared answer helpers and their tests in one
file, long to read and to review. Split it along the seams it already has, with no behaviour change: `just
check` stays green and the answers of the tools are unchanged.

# Findings (exploration of 2026-10-29, at 2a9c27a)
* Cut from `issues/split_largest_files.md` (one issue per crate, to stay under the size limit).
* `crates/retrodoc-mcp/src/tools.rs`, 1,127 lines: `Docs` and its builders at 23-128; one method of `impl Docs`
  per tool (`list_domains` 129, `get_domain` 155, `get_feature` 229, `get_use_case` 285, `read_source` 358,
  `git_log` 378, `search_docs` 389); private helpers of `Docs` at 449-526; free answer helpers at 527-646;
  tests at 647-1127 (about 480 lines).
* The routing is already apart in `server.rs`.

# Approach
1. Tests apart: `tools.rs` becomes `tools/mod.rs` (a `git mv`) plus `tools/tests.rs` (about 960 changed lines,
   the tests being dedented).
2. One tool per file: a `tools/<tool>.rs` per tool holding its own `impl Docs` block, the shared helpers in one
   file (about 1,000 changed lines; cut in two deliverables if it is over the limit at planning).
3. One commit per deliverable.

# Resources
* `issues/split_largest_files.md` (the issue it was cut from, and its plan)
* `issues/done/refactor_code_hygiene.md` (how the earlier splits were made)
* `docs/crates/render-mcp-cli.md` (module list to update)

# Hints
* after split_largest_files
* Keep the public API of the crate (re-export from `mod.rs`).
* The line numbers above move with every commit; read the file again at planning.
