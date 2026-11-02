---
priority: low
depends_on: [generated_docs_language]
why: 'a language change renames every item, so the whole previous tree stays beside the new one; reported today, but by hand to clean'
---
# Remove the generated pages no run produces any more

# Goal

When an item is renamed (a domain, a feature, a use case), the render writes its new page and leaves the old one,
reported as "Not produced by this run (left in place, delete by hand if obsolete)" because it may hold hand-written
content. After a change of `[output] language` every item is renamed, so the docs folder holds the whole previous
tree beside the new one. A page RetroDoc wrote and nobody edited since should be removed; an edited one kept and
reported as today.

# Findings

* 2026-11-02, delivery_router, deepseek-chat: after the English run, a run with `language = "fr"` left
  `functional/delivery-parties/`, `delivery-travel/` and `rider-assignment/` beside the French folders, listed under
  "Not produced by this run" (`crates/retrodoc-render/src/plan.rs`, `WritePlan::stale`).

# Approach

1. Record the hash of each page the render writes (a manifest under `_retrodoc/`), then remove a stale page whose
   content still has its recorded hash, and keep and report the others.
2. Or a `render --prune` flag that removes the stale pages on request.

# Specs

* A spec of the render if one is written; else `docs/crates/render-mcp-cli.md`.

# Milestone

Docs in the reader's language

# Resources

* `crates/retrodoc-render/src/plan.rs` (`plan`, `stale`, `summary`), `docs/crates/render-mcp-cli.md`.
