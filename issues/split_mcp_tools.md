# Split the MCP tools file into its tests and one file per tool

# Goal
`crates/retrodoc-mcp/src/tools.rs` holds every MCP tool, their shared answer helpers and their tests in one
file, long to read and to review. Split it along the seams it already has, with no behaviour change: `just
check` stays green and the answers of the tools are unchanged.

# Findings (exploration of 2026-10-29, at 2a9c27a)
* Cut from `issues/done/split_largest_files.md` (one issue per crate, to stay under the size limit).
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
* `issues/done/split_largest_files.md` (the issue it was cut from, and its plan)
* `issues/done/refactor_code_hygiene.md` (how the earlier splits were made)
* `docs/crates/render-mcp-cli.md` (module list to update)

# Hints
* after split_largest_files
* Keep the public API of the crate (re-export from `mod.rs`).
* The line numbers above move with every commit; read the file again at planning.

# Tracking

Plan: validated

## Understanding
Split `crates/retrodoc-mcp/src/tools.rs` (1,127 lines) into its tests and one file per tool, pure moves: answers and
public API (`retrodoc_mcp::tools::Docs`) unchanged. Out of scope: `server.rs` and the other modules of the crate.

## Plan
1. [x] Tests apart: `tools.rs` → `tools/mod.rs` (git mv) + `tools/tests.rs` (about 960 changed lines); verified by `just check`.
2. [x] Shared helpers and domain tools apart: `tools/answer.rs` (constants, free answer helpers, private helpers of
   `Docs` as `pub(super)`), `tools/list_domains.rs`, `tools/get_domain.rs` (about 650 changed lines); verified by `just check`.
3. [x] Remaining tools apart: `tools/get_feature.rs`, `tools/get_use_case.rs`, `tools/read_source.rs`,
   `tools/git_log.rs`, `tools/search_docs.rs`; `mod.rs` keeps `Docs` and its builders (about 480 changed lines);
   verified by `just check`.
4. [x] Fixes of the `high` review: the tests name their imports instead of taking `mod.rs`'s through
   `use super::*`; verified by `just check`.

## Decisions
- Mode: autonomous
- Base commit: 3dfec60
- File names (user, 2026-10-29): one file per tool, named after the tool.

## Deliverable 1: tests apart
- done: `tools.rs` → `tools/mod.rs` (git mv) + `tools/tests.rs` (the test module, dedented); docs point to `tools/`.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved code identical modulo whitespace | yes (script comparison; rustfmt reflowed two asserts) |
  | docs (`docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md`) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none

## Deliverable 2: shared helpers and domain tools apart
- done: `tools/answer.rs` holds the constants, the free answer helpers and the private helpers of `Docs` (now
  `pub(super)`); `tools/list_domains.rs` and `tools/get_domain.rs` hold their tool's `impl Docs` block; the test
  imports that came through `use super::*` (`RuleKind`, `SourceRef`) are named in `tests.rs`.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved lines identical (sorted line comparison) | yes, apart from the `impl`/`use`/`mod` headers and `pub(super)` |
  | docs (`docs/crates/render-mcp-cli.md`) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none

## Deliverable 3: remaining tools apart
- done: `tools/get_feature.rs`, `get_use_case.rs`, `read_source.rs`, `git_log.rs`, `search_docs.rs` each hold their
  tool's `impl Docs` block; `mod.rs` keeps `Docs`, its builders and `load`; the crate doc said "five tools", now
  "seven" (the server has seven).
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved lines identical (sorted line comparison) | yes, apart from the `impl`/`use`/`mod` headers |
  | docs (`docs/crates/render-mcp-cli.md`, `lib.rs` crate doc) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none

## Deliverable 4: fixes of the high review (`3dfec60..b71dbf0`, 10 findings)
- done: `tests.rs` imports `Docs`, the model types, `DomainMap`, `Glossary` and `Freshness` by name instead of
  through `use super::*` (finding 8: the tests depended on what `mod.rs` happens to import).
- Findings 1 to 7 and 9 are on behaviour that predates the split (code moved unchanged): filed as
  `issues/mcp_tools_answer_edge_cases.md`, ranked before `git_log_follow_renames`. Finding 10 (the issue still
  open) is the close, asked of the user.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | docs | n/a (test imports only) |
- red: none (no new test)
- Code review: no findings
- not verified: none
- decisions needed: none
