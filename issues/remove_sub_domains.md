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
