# Move the OpenRouter provider tests into files of their own

# Goal
`crates/retrodoc-llm/src/openrouter.rs` holds the provider and about 740 lines of its tests in one file, long
to read and to review. Move the tests apart, one file per theme, with no behaviour change: `just check` stays
green.

# Findings (exploration of 2026-10-29, at 2a9c27a)
* Cut from `issues/split_largest_files.md` (one issue per crate, to stay under the size limit).
* `crates/retrodoc-llm/src/openrouter.rs`, 1,281 lines: production code at 1-540; tests at 541-1281 with
  themes already apart: config and endpoints (546-650), retry delays (651-698), completion (699-986), schema and
  temperature fallbacks with their fake server (987-1178), timeouts `LTO-1` to `LTO-4` (1179-1281).
* The production code has no seam worth a split (decision of the planning of `split_largest_files`, 2026-10-29).

# Approach
1. `openrouter.rs` becomes `openrouter/mod.rs` (a `git mv`) plus `openrouter/tests/`: a `mod.rs` with the shared
   helpers and one file per theme (config, retry, completion, fallbacks, timeouts).
2. About 1,480 changed lines, the tests being dedented: two deliverables, the helpers with config and retry
   (about 500), then completion, fallbacks and timeouts (about 980).
3. One commit per deliverable.

# Resources
* `issues/split_largest_files.md` (the issue it was cut from, and its plan)
* `issues/done/refactor_code_hygiene.md` (how the earlier splits were made)
* `docs/crates/llm.md` (module list to update)
* `specs/` (the `// spec: LTO-n` lines move with their tests; `just check-specs` must still find them)

# Hints
* after split_render_markdown
* Keep the public API of the crate (re-export from `mod.rs`).
* The line numbers above move with every commit; read the file again at planning.
