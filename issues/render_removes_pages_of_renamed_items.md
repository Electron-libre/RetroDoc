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

* `specs/docs-pruning.md` (RND).

# Milestone

Docs in the reader's language

# Resources

* `crates/retrodoc-render/src/plan.rs` (`plan`, `stale`, `summary`), `docs/crates/render-mcp-cli.md`.

# Tracking

Plan: validated

## Understanding
`generate` and `render` record the hash of each page they write under `functional/` (`_retrodoc/rendered-pages.json`)
and remove a page no run produces any more when it still has its recorded hash; an edited or unrecorded page is kept
and reported as today. Approach 1 of the issue; no `--prune` flag.

## Plan
1. [x] Record and prune: the record of the hashes in `plan`, the removal in `WritePlan::apply`, a "Removed" section in
   the preview, empty folders removed; verified by tests of `plan.rs` and a CLI test that renames a domain in the saved
   artifacts then runs `render` and `render --dry-run`; makes true RND-1 to RND-6.

## Decisions
- Mode: autonomous
- Base commit: 0b00c703bb396d63435bd496c49aefcd28e8c46e
- Pages with no recorded hash (written before the record): kept and reported; no `--prune` flag.

## Deliverable 1

- done: `plan` records `_retrodoc/rendered-pages.json` and splits the pages no longer produced into `removed`
  (recorded hash matches) and `stale`; `apply` removes the first and their empty folders; the preview lists
  `remove  <path>`; the CLI prints the count.
- red: RND-1, RND-2, RND-4, RND-5: `docs/_retrodoc/rendered-pages.json` not found, then `functional/orders` still
  there and listed under "Not produced by this run". RND-3 and RND-6 were green before the code (they guard the
  kept pages and the no-op rerun).
- Preview: a `remove  <path>` line among `create`/`update` instead of a "Removed" heading (RND-2 amended).
- not verified: a real run on delivery_router (English then French).
