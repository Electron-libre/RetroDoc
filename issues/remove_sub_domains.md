# Remove the sub-domains that no path fills any more

# Goal
Only the clustering by directory of the removed `files` path (ADR 0035) made sub-domains: the `surface` and
`contexts` domains are flat and every feature they make has no `sub_domain_slug`. The sub-domain model, its
render, its report lines, its MCP fields and its benchmark handling are now dead code that every change of
those crates still has to carry. Remove them, or keep what reading an old `domains.yaml` or `features.yaml`
needs, with the docs.

# Findings (inventory of 2026-10-28, `issues/done/single_pipeline_path.md` deliverable 4)
* `DomainCluster.sub_domains` and `SubDomainCluster` (`retrodoc-pipeline` `domains/mod.rs`), `Feature.sub_domain_slug`
  and the sub-domains of `retrodoc_core::model` (`model.rs`).
* Readers: `retrodoc-render` `markdown.rs` (14 mentions, the sub-domain sections and pages), `report.rs` (8),
  `benchmark/metrics.rs` (8), `retrodoc-mcp` `tools.rs` and `corpus.rs` (7 each), `benchmark/judge.rs`,
  `matching.rs`, `summary.rs`, `retrodoc-cli` `commands/docs.rs`, `features/from_surface.rs` (`unit_key`, `save_partial`).
* The surface features set `sub_domain_slug: None` (`features/from_surface.rs`).
* Many test fixtures build `sub_domains: Vec::new()` or `sub_domain_slug: None`: the change is wide but mechanical.

# Approach
1. Decide what an old artifact that holds sub-domains becomes (read and ignored, or the field kept with
   `#[serde(default)]` and never written).
2. Remove the model fields, then their readers crate by crate (render, MCP, report, benchmark), the docs of
   `docs/crates/` and the rendered pages that described sub-domains.
3. Check the render stays idempotent on a saved run (a second `generate` writes nothing).

# Resources
* ADR 0035, ADR 0010 (deterministic render), `docs/crates/render-mcp-cli.md`, `docs/crates/pipeline.md`.

# Hints
* A rendered page set saved before may hold sub-domain pages: the render plan would delete them on the next
  run, which changes the docs of a repository documented with the `files` path.
* Do it after `issues/done/remove_repo_map.md`; it touches the render and the MCP, which `pass_runner_and_run_context`
  does not, so it can wait behind it.

# Tracking

Plan: validated

## Understanding
Remove the sub-domain model (`Domain.sub_domains`, `SubDomain`, `Feature.sub_domain_slug`, `DomainCluster.sub_domains`,
`SubDomainCluster`) and every reader (render, MCP, report, benchmark metrics), with the docs. No behaviour of the
`surface`/`contexts` paths changes; old artifacts keep loading (serde ignores the unknown fields, no `deny_unknown_fields`
on these types), the features fingerprint key stays `<domain>/-` so a rerun stays a no-op.

## Plan
1. [x] Pipeline side: drop `DomainCluster.sub_domains`, `SubDomainCluster`, the sub-domain loop of `layer_named_domains`,
   `RunMetrics.sub_domains` and the `sub-domain(s)` of the benchmark summary; `unit_key` loses its sub-domain argument
   (key unchanged); verified by a test that an old `domains.yaml` holding `sub_domains` still loads, and the updated
   benchmark tests; ~250 lines.
2. [x] Core model and readers: drop `Domain.sub_domains`, `SubDomain`, `Feature.sub_domain_slug`; the render loses the
   sub-domain sections and paths, the MCP its `billing/payment` addresses and sub-domain listing, the report its
   sub-domain confidences; fixtures updated; docs (`README.md`, `PLAN.md` §2, `docs/crates/render-mcp-cli.md`,
   `docs/crates/pipeline.md`); verified by a test that an old `features.yaml` holding `sub_domain_slug` still loads,
   `just check`, and `retrodoc render` run twice on a copy of a saved run of `~/retrodoc-runs` (LLM-free: the
   second writes nothing); ~600 lines.

## Decisions
- Mode: autonomous
- Base commit: cf3103fc3aacb3b2148e3ef33baf57324d975bd2
- Old artifacts: read, the sub-domain fields ignored (no `#[serde(default)]` field kept). A rendered page set saved
  before keeps its sub-domain pages: the next render writes the pages under `functional/<domain>/` and reports the
  old ones as stale (the render plan never deletes, they may hold hand-written content). Only a repo documented with
  the removed `files` path had any, and its features are already dropped (ADR 0035). The files of an old
  `sub_domains[].paths` are dropped with it, not moved to the domain (same reason).
- No spec: pure removal of dead code, no spec names sub-domains.

## Deliverable 1 summary
- done: `DomainCluster.sub_domains`, `SubDomainCluster`, `RunMetrics.sub_domains` and the `sub-domain(s)` of the
  benchmark summary line removed; `unit_key` takes the domain only, key still `<domain>/-`; `domain_models` writes
  an empty `sub_domains` until deliverable 2 removes the field.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (warnings predate the change) |
| docs | n/a: no doc names the removed fields |
| code review | low, see below |

- red: none (no spec); the new load test failed with `sub_domains: []` written on save.
- Code review: no findings
- not verified: none.
- decisions needed: none.

## Deliverable 2 summary
- done: `Domain.sub_domains`, `SubDomain`, `Feature.sub_domain_slug` removed; the render puts every feature page under
  `functional/<domain>/`, the MCP addresses a domain by its slug only (`get_domain("billing/x")` is unknown), the
  report gives the domain confidence only; docs updated.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (warnings predate the change) |
| docs | `README.md`, `PLAN.md` §3 and §5, `docs/crates/render-mcp-cli.md` |
| code review | low, see below |

- red: none (no spec); the new load test failed with `sub_domain_slug` written on save.
- Code review: no findings
- Render check: on a copy of a saved `files`-path run, the first `render` writes the pages under
  `functional/<domain>/` and reports the sub-domain ones as stale, the second writes nothing.
- not verified: none.
- decisions needed: none. The prompt of the contexts pass keeps "business sub-domains" (a word of the prompt, not the
  model; changing it would change the answers).

3. [x] Fixes of the `high` review: one `feature_id` in the MCP (`corpus.rs`), a test that `get_domain` takes a slug
   alone, `container` inlined and `direct` renamed `business` in the render, the issue text corrected on stale pages.

## Deliverable 3 summary
- done: the review fixes above; the commit message of deliverable 2 says the old pages "move": they are reported
  as stale, not deleted.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass |
| docs | n/a: no doc describes these helpers |
| code review | low, see below |

- red: none (refactor; the new MCP test passes on the code of deliverable 2).
- Code review: no findings
- High review findings not fixed: `get_domain` no longer resolves `billing/x` to `billing` (a domain id is a slug,
  as `list_domains` shows); the paths of an old `sub_domains` are not moved to their domain (files path only).
- not verified: none.
- decisions needed: none.
