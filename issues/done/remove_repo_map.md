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

# Tracking

Plan: validated

## Understanding
Delete the repo map (`repo_map/`, `RepoMapCache`, `Artifact::RepoMap`) that no pass calls since ADR 0035, keeping
its two shared helpers. Out of scope: the `"repo-map"` fixtures of old `usage.json` runs, the `[llm.passes.repo-map]`
removed-setting error of `config.rs`, the ADRs (historical records).

## Plan
1. [x] Move the helpers: `read_file_lossy` and `truncate_chars` go to a new `src/text.rs` (pipeline-private), the
   eight importers edited; verified by `just check` (no behaviour change); ~40 lines.
2. [x] Remove the repo map: `repo_map/`, the `lib.rs` exports, `RepoMapCache` (keeping `hash_content`),
   `Artifact::RepoMap` and its copy in `benchmark/frozen.rs`, the MCP freshness test that used the cache; RER-2
   amended and proven by a new use cases test (two features, one file changed, only its feature redone), spec back
   to `accepted`; verified by `just check`; makes true RER-2; ~1,250 lines, almost all deletions (see Decisions).
3. [x] Docs: `lib.rs` and `source_hashes.rs` doc comments, `retrodoc-ingest` and `retrodoc-llm` crate docs,
   `docs/crates/pipeline.md`, `docs/ARCHITECTURE.md`, `docs/dev/commands.md`, `specs/pipeline-settings.md` if it
   still names it, PLAN; verified by `skill:update-docs` and `just check`; ~100 lines.
4. [x] Fixes of the `high` review of `18cb5fb..HEAD`: stale docs, the `cache.rs` doc, the RER-2 test made
   independent of the empty-answer retry; verified by `just check`.

## Decisions
- Mode: autonomous
- Base commit: 18cb5fbb9a531ad409ceb0a831b4546652365417
- An old `.retrodoc/cache/repo-map.json` is left on disk: nothing reads it, `FrozenInputs::prepare` copies it harmlessly.
- Deliverable 2 is slightly over the ~1,000-line limit but is a pure deletion of a self-contained module plus one
  test: kept whole, since splitting `repo_map/` would leave a module that no longer compiles alone.

## Deliverable summaries

### 1. Move the helpers
- done: `read_file_lossy` and `truncate_chars` live in `src/text.rs`; the nine importers point there.
- check | result: tests ok, clippy ok, fmt ok, specs ok, `check-harness` fails on the git review gate hook (the
  uncommitted switch of `.claude/harness.toml` to `cardinal`, not this change), code review below.
- red: none (no behaviour change, no new test).
- not verified: nothing.
- decisions needed: none.
- Code review: no findings.

### 2. Remove the repo map
- done: `repo_map/`, `RepoMapCache`, `Artifact::RepoMap`, the MCP test and the `lib.rs` exports removed;
  `cache.rs` keeps `hash_content` only; `complete_text` and `Progress::finish`, used only by the repo map, removed;
  RER-2 proven by `a_changed_file_redoes_only_the_feature_that_reads_it`; `specs/rerun.md` back to `accepted`.
- check | result: tests ok, clippy ok, fmt ok, specs ok, `check-harness` fails as in deliverable 1.
- red: RER-2: none, the behaviour already existed (the test pins it; its first run failed only on a wrong call
  count of the test itself, an empty answer is asked twice).
- not verified: nothing.
- decisions needed: none.
- Code review: no findings.

### 3. Docs
- done: crate docs of `retrodoc-ingest` (`lib.rs`, `git_history.rs`, `existing_docs.rs`) and `retrodoc-llm`, the
  `retrodoc-pipeline` `lib.rs`, `source_hashes.rs` and MCP `freshness.rs` comments, the `progress.rs` test label,
  `docs/crates/pipeline.md` (row removed), `docs/ARCHITECTURE.md` §8 diagram, `docs/dev/commands.md` example,
  PLAN (roadmap item 2 marked removed, `.erb` and truncation notes). The ADRs and PLAN §7.2 stay as history.
- check | result: tests ok, clippy ok, fmt ok, specs ok, `check-harness` fails as in deliverable 1.
- red: none (docs only).
- not verified: nothing.
- decisions needed: none.
- Code review: no findings.

### 4. Fixes of the high review
- done: `docs/crates/pipeline.md` (pass order without the repo map, no `complete_text`), PLAN §7.4 truncation line,
  `cache.rs` doc (what the hash keys), the RER-2 test answers a use case (one call per feature, no RER-4 retry);
  follow-up `issues/done/shared_lossy_read_and_one_pass_truncation.md` queued at rank 2.
- check | result: tests ok, clippy ok, fmt ok, specs ok, `check-harness` fails as in deliverable 1.
- red: none (fixes of docs and of a test that already passed).
- High review: 10 findings. Fixed: stale `pipeline.md` pass order and `complete_text`, PLAN truncation line,
  `cache.rs` doc, RER-2 test bound to the RER-4 retry. Not fixed: `--force` leaves `repo-map.json` (decision
  recorded above: left on disk); MCP test not retargeted (freshness reads `source-hashes.json` only, no pass hash
  to retarget it to); `check-harness` red (the user's uncommitted harness switch); `truncate_chars` double walk
  and the hand-written lossy reads (outside this issue, filed as a follow-up); PLAN line on `llm.concurrency`
  "repo map only" kept (history of phase 8).
- not verified: nothing.
- decisions needed: none.
- Code review: no findings.
