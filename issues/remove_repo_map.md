# Remove the repo map and its cache

# Goal
Since ADR 0035 `generate` no longer builds the repo map (the LLM summaries of every file and directory of the
removed `files` path), but `retrodoc-pipeline` still holds it, its summary cache and its artifact. Remove them,
with the docs and the spec lines that describe them, keeping `just check` green and a second `generate` a
no-op. Split from `issues/done/single_pipeline_path.md` for the size limit.

# Findings (inventory of 2026-10-28, `issues/done/single_pipeline_path.md`)
* `repo_map/` (`mod.rs`, `summarize.rs`, `tests.rs`): about 1,020 lines; `build_repo_map`, `estimate_repo_map`,
  `RepoMap`, `FileSummary`, `ModuleSummary`, `RepoMapOptions`, `RepoMapEstimate`, re-exported by `lib.rs`.
* Two helpers of `repo_map/mod.rs` are used elsewhere and must move first: `read_file_lossy` (actors, chunk
  check, slices, ranking, glossary, entry points, business read) and `truncate_chars` (benchmark judge and
  rules replay).
* `cache.rs`: `RepoMapCache` is repo-map only (about 150 lines); `hash_content` stays (sources, source hashes,
  fingerprints, MCP freshness and others).
* `Artifact::RepoMap` (`artifact.rs`) and its copy in `benchmark/frozen.rs`.
* `retrodoc-mcp/src/freshness.rs`: one test uses `RepoMapCache` to show that a repo-map update does not count;
  MCP freshness itself reads `source-hashes.json`.
* `specs/rerun.md` RER-2 speaks of file and folder summaries, traced only by the repo map tests; RER-1 and
  RER-3 also have traces there.
* Docs still naming the repo map: `crates/retrodoc-pipeline/src/lib.rs` and `source_hashes.rs` doc comments,
  `retrodoc-ingest` and `retrodoc-llm` crate docs, `docs/crates/pipeline.md`, `docs/dev/commands.md` (a test
  name used as example), PLAN.
* `"repo-map"` in the token accounting tests and the `behaviour_baseline` example are fixtures of old
  `usage.json` runs: they can stay.

# Approach
1. Move `read_file_lossy` and `truncate_chars` to a shared module (the imports edited, no file moved).
2. Remove `repo_map/`, `RepoMapCache`, `Artifact::RepoMap`, the MCP test, the `lib.rs` exports.
3. Reword RER-2 to the units that remain (a changed entry point or file redoes its own unit), check RER-1 and
   RER-3 keep a trace; update the docs.
4. Decide what an old `.retrodoc/cache/repo-map.json` becomes (left on disk, or deleted by `generate --force`).

# Specs
* `specs/rerun.md`: RER-2 amended.

# Resources
* ADR 0035, ADR 0005 (incremental rerun), `issues/done/single_pipeline_path.md` (`# Tracking`).
* `crates/retrodoc-pipeline/src/repo_map/`, `cache.rs`, `artifact.rs`, `benchmark/frozen.rs`,
  `crates/retrodoc-mcp/src/freshness.rs`.

# Hints
* Do it after `issues/done/single_pipeline_path.md` (which removes the last callers of the repo map) and before
  `issues/pass_runner_and_run_context.md`: less code to refactor.
* The saved frozen inputs of the per-pass replays may hold a `repo-map.json`: `FrozenInputs::prepare` must keep
  working on them.
