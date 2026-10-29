# Split the largest source files along what they already hold

# Goal
Five files hold several things each (production code, its tests, several tools or pages), which makes them long
to read and to review. Split each along the seams it already has, with no behaviour change: `just check` and a
second `generate` writing nothing stay green.

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
2. Then, where it reads better: one MCP tool per file, one page per file of the render, one section per file of
   the report, chunking and excerpts apart.
3. One commit per file. A move that also edits counts twice in the size limit: about 4,600 lines moved in all,
   so more than one issue or several small deliverables; propose the cut at planning.

# Resources
* `issues/done/refactor_code_hygiene.md` (how the earlier splits were made)
* `issues/pass_runner_and_run_context.md` (origin: its point 5, left out for size)
* `docs/crates/render-mcp-cli.md`, `docs/crates/llm.md`, `docs/crates/pipeline.md` (module lists to update)

# Hints
* Keep the public API of each crate (re-export from `mod.rs`).
* The line numbers above move with every commit; read the files again at planning.
