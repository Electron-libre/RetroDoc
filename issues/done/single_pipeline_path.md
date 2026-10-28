# Keep one pipeline path and remove the other

# Goal
`generate` has two ways to build the domains and the features: from the file summaries (`domains_from =
"files"`, the default) and from the entry points and entities (`"surface"`). ADR 0020 supersedes ADR 0004,
yet both paths are kept, documented, tested and smoke-tested, and every change of a pass downstream must
hold on both. Once the default is decided, remove what the losing path alone uses, or state in an ADR why
both stay and for which repositories.

# Findings (project review of 2026-10-12)
* `generate/mod.rs` branches on `config.pipeline.domains_from` (`features_from_surface` /
  `features_from_files`).
* Used by the `files` path only, as far as the review saw (to confirm by reading the callers): `repo_map/`
  (LLM summaries per file and directory), `domains/mod.rs`, `coverage.rs`, `repair.rs`, `features/mod.rs`,
  `estimate_repo_map`, `llm.batch_chars`. `ranking.rs` (`--max-files`) drops files before the repo map.
* Used by the `surface` path only: `domains/from_surface.rs`, `features/from_surface/`,
  `behaviour_coverage.rs`.
* The repo map is 133 to 138 calls and about 114k tokens in on linkding
  (`issues/done/behaviour_coverage_domains.md`), spent to cluster the domains.
* The decision is parked (`issues/done/surface_domains_default_decision.md`, user, 2026-10-10); on a 9-file library
  the `surface` features are worse, because the entry points pass lists constructors and internal
  collections there.
* `retrodoc-pipeline` is 23.8k of the workspace's 37.9k lines.

* `domains_from` has a third value since 2026-10-13, `"contexts"` (ADR 0029, `generate/contexts.rs`): the surface
  pipeline run a bounded context at a time. It shares the passes of the `surface` path and adds `Retention`,
  per-domain placement and vocabulary, `contexts/`; the decision must say what happens to it (it is the way to
  run a large application).

# Approach
1. Wait for `surface_domains_default_decision` (fed by `validate_on_large_application`).
2. List what each path alone uses (modules, config keys, artifacts, commands, docs, skills, tests), and what
   else reads the repo map (MCP freshness reads `content_hash`, the report, the ranking).
3. Candidates: (a) `surface` only, the repo map and the clustering by directory removed; (b) `surface` by
   default, `files` kept as a fallback when the surface is empty (no role rules, no entry point), with the
   reduced code that fallback needs; (c) both kept, chosen by the kind of repository (application or
   library), written in an ADR.
4. Remove in steps that each keep `just check` green and a second `generate` a no-op; an ADR supersedes
   what it replaces.

# Resources
* `crates/retrodoc-cli/src/commands/generate/mod.rs`
* `crates/retrodoc-pipeline/src/repo_map/`, `domains/`, `features/`, `ranking.rs`, `behaviour_coverage.rs`
* ADRs 0004, 0011, 0017 and 0020; `issues/done/surface_domains_default_decision.md`
* Skill `update-docs`

# Hints
* A removed path is a breaking change for a repository whose `retrodoc.toml` names it: decide what an old
  value does (error with a message, or ignored).
* The `files` path is the only one with the "every file in a domain" coverage; the `surface` path reports
  the unreached files as technical support instead.
* Do this before `pass_runner_and_run_context`: less code to refactor.

# Tracking

Plan: validated

## Understanding
Candidate (a): `generate` keeps the `surface` path (and `contexts`); the `files` path and what only it uses go,
an old `domains_from = "files"` is refused with a message (SET-8), ADR 0035 supersedes 0004 and 0017. Out of
scope: `--max-files` and `behaviour_coverage.rs` (shared, not files-only as the findings thought), the dead
sub-domain code (follow-up issue). The repo map itself goes in a second issue (`remove_repo_map`, size limit).

## Plan
1. [x] Retire the `files` entry: `DomainsFrom::Files`, `[llm] batch_chars` and `[llm.passes.repo-map]` refused
   with a message (SET-8), the `else` branch of `generate` with `features_from_files`, `cluster_domains`,
   `build_map` and their `print` helpers, SET-7's warning text, `--domains-from files` of the `smoke` and
   `benchmark` skill scripts, docs (README, ARCHITECTURE, `docs/crates/pipeline.md`, PLAN), ADR 0035 accepted,
   0004/0017/0011/0034 statuses; verified by the SET-6/7/8 config tests and the skill script tests; makes true
   SET-6, SET-7, SET-8 (~800 lines).
2. [x] Remove the features per domain from the file summaries (`build_features` and its files-only helpers of
   `features/mod.rs`, their tests; the shared `slugify`, `save_partial`, `unit_key`, load/save stay); verified by
   `just check` (~735 lines, deletion).
3. [x] Remove the placement of unassigned files (`domains/repair.rs`, its call in `build_domains`, its tests;
   ADR 0017); verified by `just check` (~300 lines, deletion).
4. [x] Remove the clustering by directory (`build_domains`, `CoverageReport`, `domains/coverage.rs`, the
   clustering prompt, files-only tests of `domains/tests.rs`; `layer_named_domains` stays); verified by
   `just check` and a `generate` on a copy of a saved `delivery_router` `surface` run making no LLM call
   (~900 lines, deletion).
5. [x] Name the removed setting first (found by the check of deliverable 4): only a missing `retrodoc.toml` asks
   for `init`; verified by `crates/retrodoc-cli/tests/removed_settings.rs`; makes true SET-8 through the binary.
6. [x] Fixes of the `high` review of the issue (10 findings): replays on frozen inputs of `files` runs, a killed
   run bringing back `files` features, PLAN §2, a stale comment, explicit imports of `features/from_surface`;
   verified by the frame and features tests.

## Decisions
- Mode: autonomous
- Base commit: a4cc7a6cc02faa83db91ce3eb06cbf8abd3fd3c8
- Candidate (user, 2026-10-28): (a), `surface` only, the `files` path removed (ADR 0035).
- Old `files` settings (user, 2026-10-28): refused with a message (SET-8), not ignored.
- Saved `files` clones (user, 2026-10-28): `report` and `benchmark --judge` keep refusing them (SET-8);
  delete the `domains_from` line in the clone, or use `--saved-judge`.
- Split (user, 2026-10-28): the repo map goes in a new issue `remove_repo_map`, queued right after this one;
  the dead sub-domain code gets a follow-up issue at deliverable 4.

## Deliverable 1
- done: `generate` has no `files` branch (`features_from_files`, `cluster_domains`, `build_map` and their prints
  gone); `DomainsFrom::Files`, `batch_chars`, the `repo-map` pass name gone; a config naming
  `domains_from = "files"`, `batch_chars` (in `[llm]` or a pass) or `[llm.passes.repo-map]` is refused with a
  message (SET-8), checked before the parse, `body_lines` too; the empty-surface warning no longer names `files`
  (SET-7); `just smoke` and `just benchmark` refuse `--domains-from` (the benchmark's `patch-config` loses its
  domains-from argument); ADR 0035 accepted, statuses of 0004, 0011, 0017, 0034; README, ARCHITECTURE (pipeline
  diagram and table, safety nets, rerun diagram, disk layout), `docs/crates/pipeline.md`, `llm.md`,
  `render-mcp-cli.md`, PLAN.
- checks: fmt, clippy, tests, specs green; `check-harness` red on `modified: native review gate`, from the
  harness migration in progress (`.claude/harness.toml` source changed before this session), not this diff.
- red: SET-8: `called Result::unwrap_err() on an Ok value` (a config with `domains_from = "files"` loaded);
  SET-7: the warning still named `domains_from = "files"`; the skill script tests failed on
  `--domains-from is refused` (smoke and benchmark).
- not verified: a whole `generate` (no LLM run in this deliverable); the pipeline functions of the `files` path
  are now unused, removed by deliverables 2 to 4.
- Code review: 1 findings: the `files` functions are still exported by `retrodoc-pipeline` with no caller
  outside their tests; not fixed here, by plan (deliverables 2 to 4, then `remove_repo_map`).
- Found: `test_smoke.rs` printed `FAIL` for the inline checks of `--domains-from` without failing the script
  (they were not in `failed`); the new check is.

## Deliverable 2
- done: `build_features` and its files-only helpers (prompt, units, reuse, validation) and their tests removed
  from `features/`; `load_features`, `save_features`, `unit_key`, `save_partial`, `slugify`, `unique_slug`
  stay; the `features/` row of `docs/crates/pipeline.md`.
- checks: fmt, clippy, tests, specs green (RER-1, 2, 3, 5 keep traces in the use cases, rules and surface
  features tests); `check-harness` red as in deliverable 1 (harness migration in progress).
- red: none (a deletion, no new behaviour).
- Code review: no findings.
- not verified: a whole `generate` (no LLM run).

## Deliverable 3
- done: `domains/repair.rs`, its call in `build_domains` and its four tests removed (ADR 0017); the module doc
  and the `domains/` row of `docs/crates/pipeline.md`.
- checks: fmt, clippy, tests, specs green; `check-harness` red as before (harness migration in progress).
- red: none (a deletion, no new behaviour).
- Code review: no findings.
- not verified: a whole `generate` (no LLM run).

## Deliverable 4
- done: `build_domains`, `CoverageReport`, `domains/coverage.rs`, the clustering prompt and fingerprint and
  their tests removed; `domains/prompt.rs` keeps `layer_named_domains`; `from_surface.rs` names the imports it
  took from `super::*`. Issue `remove_sub_domains` created and queued after `pass_runner_and_run_context`.
- checks: fmt, clippy, tests, specs green; `check-harness` red as before (harness migration in progress); a
  `generate --no-confidence` on a copy of a saved `delivery_router` `surface` benchmark run of 2026-10-28
  (deepseek-chat, `base_url` pointed at a closed port) made no LLM call and wrote no file
  (`~/retrodoc-runs/single_pipeline_path-noop`).
- red: none (a deletion, no new behaviour).
- Code review: no findings.
- not verified: a `generate` from scratch (no LLM run).

## Deliverable 5
- done: a command on a `retrodoc.toml` naming a removed setting printed `config not found — run retrodoc init
  first` above the SET-8 message; only a missing file asks for `init` now (`workspace.rs`). Found by running
  the copy of the `files` run of 2026-10-28, which now stops with the SET-8 message alone. Cut from
  deliverable 4 for the size limit, reviewed on its own.
- checks: as deliverable 4.
- red: SET-8: `removed_settings.rs` failed with `Error: config not found — run retrodoc init first` as the
  first line.
- Code review: no findings.

## Deliverable 6 (fixes of the `high` review, 10 findings)
- done: `tree_evidence` reads a saved run with `Config::load_ingest_and_output` (only `[ingest]` and
  `[output]`), so `pass_replay` keeps working on the frozen inputs, whose source runs name
  `domains_from = "files"`; `save_partial` never puts back a feature with no entry point (one of the `files`
  path), so a killed first run after the switch does not mix them with the surface features; `unit_key` and
  `save_partial` moved to `features/from_surface.rs`, which names its imports instead of `super::*`; PLAN §2
  lists the surface pipeline; the stale `--domains-from` comment of the SET-6 test;
  `docs/crates/pipeline.md` on the frozen inputs.
- Not fixed: `report` and `benchmark --judge` refuse a saved clone of a `files` run (SET-8 as written; the
  line can be deleted in the clone, `--saved-judge` still works): decision left to the user; `unit_key` and
  `save_partial` still handle a sub-domain slug (changing the keys would redo every saved unit; with
  `remove_sub_domains`); `freeze` still copies a `repo-map.json` and `lib.rs` still exports the repo map
  (`remove_repo_map`); `removed_setting` special cases instead of one table (four shapes, one each); the second
  TOML parse of every load (a `[llm] batch_chars` loads without error, so it must be looked for on success too).
- checks: fmt, clippy, tests, specs green; `check-harness` red as before (harness migration in progress).
- red: `a_saved_run_of_the_removed_files_path_still_gives_its_evidence`: `InvalidReference { reason: "[llm]
  batch_chars was removed: ..." }`; `a_killed_run_does_not_bring_back_the_features_of_the_removed_files_path`:
  `left: ["orders", "billing"], right: ["orders"]` (checked by removing the filter after the fix).
- not verified: `pass_replay` itself on `~/retrodoc-runs/frozen` (it calls the LLM).
- Code review: no findings.
