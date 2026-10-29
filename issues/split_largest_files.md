# Split the largest source files along what they already hold

# Goal
Five files hold several things each (production code, its tests, several tools or pages), which makes them long
to read and to review. Split each along the seams it already has, with no behaviour change: `just check` and a
second `generate` writing nothing stay green. This issue covers the two files of `retrodoc-pipeline`
(`report.rs`, `chunks.rs`); the other three were cut into `issues/split_mcp_tools.md`,
`issues/split_render_markdown.md` and `issues/split_openrouter_tests.md` (one issue per crate, for the size
limit).

# Findings (exploration of 2026-10-29, at aee4cc9)
* `crates/retrodoc-mcp/src/tools.rs`, 1,144 lines: tests at 678-1144; one block per tool (`list_domains`
  129-164, `get_domain` 165-252, `get_feature` 253-308, `get_use_case` 309-381, `read_source` 382-401,
  `git_log` 402-412, `search_docs` 413-472), shared answer helpers at 473-677. The routing is already apart in
  `server.rs`.
* `crates/retrodoc-llm/src/openrouter.rs`, 1,281 lines: tests at 541-1281 (config, retry, completion, schema and
  temperature fallbacks, timeouts).
* `crates/retrodoc-render/src/markdown.rs`, 1,174 lines: tests at 607-1174; one block per page (agent index,
  domain, feature, use case).
* `crates/retrodoc-pipeline/src/report.rs`, 1,161 lines: tests at 680-1161; one block per section (set aside,
  reconciliation, behaviour, uncovered) and `to_markdown`.
* `crates/retrodoc-pipeline/src/chunks.rs`, 597 lines: chunking (boundaries, `split_chunks`) and excerpts
  (`Focus`, `excerpt`, pieces) side by side; tests at 391-597.

# Approach
1. Tests apart first: `x.rs` becomes `x/mod.rs` (a `git mv`) plus `x/tests.rs`, as the pipeline modules already
   are (`issues/done/refactor_code_hygiene.md`, deliverable 7).
2. Then, where it reads better: one section per file of the report, chunking and excerpts apart.
3. One commit per deliverable. A move that also edits counts twice in the size limit.

# Resources
* `issues/done/refactor_code_hygiene.md` (how the earlier splits were made)
* `issues/done/pass_runner_and_run_context.md` (origin: its point 5, left out for size)
* `docs/crates/render-mcp-cli.md`, `docs/crates/llm.md`, `docs/crates/pipeline.md` (module lists to update)

# Hints
* Keep the public API of each crate (re-export from `mod.rs`).
* The line numbers above move with every commit; read the files again at planning.

# Tracking

Plan: validated

## Understanding
Split `report.rs` and `chunks.rs` of `retrodoc-pipeline` into their tests and their seams, pure moves, public API
unchanged. Out of scope: the MCP tools, the Markdown render and the OpenRouter tests (their own issues).

## Plan
1. [x] Report tests apart: `report.rs` → `report/mod.rs` + `report/tests.rs` (both test modules); verified by `just check`.
2. [x] Report sections apart: set aside, reconciliation, behaviour, uncovered in their own files of `report/`; verified by `just check`.
3. [x] Chunks tests apart: `chunks.rs` → `chunks/mod.rs` + `chunks/tests.rs`; verified by `just check`.
4. [x] Chunking and excerpts apart: `Focus`, `excerpt` and the pieces in `chunks/excerpt.rs`; verified by `just check` and a second `generate` writing nothing.

## Decisions
- Mode: autonomous
- Base commit: 2a9c27a
- Split (user, 2026-10-29): one issue per crate; the OpenRouter tests go to one file per theme under `openrouter/tests/`.

## Deliverable 1: report tests apart
- done: `report.rs` → `report/mod.rs` (git mv) + `report/tests.rs` + `report/set_apart_tests.rs` (the two test modules kept apart); docs point to `report/`.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved code identical modulo whitespace | yes (script comparison) |
  | docs (`docs/crates/pipeline.md`, `docs/ARCHITECTURE.md`) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none

## Deliverable 2: report sections apart
- done: `report/behaviour.rs`, `report/reconciliation.rs`, `report/set_aside.rs` each hold their `with_*` builder, their `write_*` section (now `pub(super)`) and their list caps; `mod.rs` keeps the model, `build_report` and `to_markdown`; the test imports that came through `use super::*` are named in `tests.rs`.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved lines identical (sorted line comparison) | yes, apart from the `impl`/`mod` headers |
  | docs (`docs/crates/pipeline.md`) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none

## Deliverable 3: chunks tests apart
- done: `chunks.rs` → `chunks/mod.rs` (git mv) + `chunks/tests.rs`; docs point to `chunks/`.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved code identical modulo whitespace | yes (script comparison) |
  | docs (`docs/crates/pipeline.md`) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none

## Deliverable 4: chunking and excerpts apart
- done: `chunks/excerpt.rs` holds `Focus`, `Piece`, `Splitter::excerpt` and `pieces`, the excerpt chunk size and the omitted marker, with the part of the module doc about excerpts; `mod.rs` re-exports `Focus`; `tests.rs` imports `fmt::Write` itself.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved lines identical (sorted line comparison) | yes, apart from `mod excerpt;` and the re-export |
  | docs (`docs/crates/pipeline.md`) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: the second `generate` writing nothing on a real repository (no run of the current code to reuse; a fresh one is a local run of hours or a paid one). The rerun tests (`specs/rerun.md`) pass, and the moves change no code of the passes.
- decisions needed: none
