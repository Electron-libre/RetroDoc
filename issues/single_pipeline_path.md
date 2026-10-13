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
* The decision is parked (`issues/surface_domains_default_decision.md`, user, 2026-10-10); on a 9-file library
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
* ADRs 0004, 0011, 0017 and 0020; `issues/surface_domains_default_decision.md`
* Skill `update-docs`

# Hints
* A removed path is a breaking change for a repository whose `retrodoc.toml` names it: decide what an old
  value does (error with a message, or ignored).
* The `files` path is the only one with the "every file in a domain" coverage; the `surface` path reports
  the unreached files as technical support instead.
* Do this before `pass_runner_and_run_context`: less code to refactor.
