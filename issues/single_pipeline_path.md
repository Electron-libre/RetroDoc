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
2. [ ] Remove the features per domain from the file summaries (`build_features` and its files-only helpers of
   `features/mod.rs`, their tests; the shared `slugify`, `save_partial`, `unit_key`, load/save stay); verified by
   `just check` (~735 lines, deletion).
3. [ ] Remove the placement of unassigned files (`domains/repair.rs`, its call in `build_domains`, its tests;
   ADR 0017); verified by `just check` (~300 lines, deletion).
4. [ ] Remove the clustering by directory (`build_domains`, `CoverageReport`, `domains/coverage.rs`, the
   clustering prompt, files-only tests of `domains/tests.rs`; `layer_named_domains` stays); verified by
   `just check` and a `generate` on a copy of a saved `delivery_router` `surface` run making no LLM call
   (~900 lines, deletion).

## Decisions
- Mode: autonomous
- Base commit: a4cc7a6cc02faa83db91ce3eb06cbf8abd3fd3c8
- Candidate (user, 2026-10-28): (a), `surface` only, the `files` path removed (ADR 0035).
- Old `files` settings (user, 2026-10-28): refused with a message (SET-8), not ignored.
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
