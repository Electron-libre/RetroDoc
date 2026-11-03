---
priority: low
depends_on: []
why: 'edge cases of the render no run has shown yet (high review of `split_render_markdown`, 2026-10-29)'
---
# Fix the edge cases and the duplicated pieces of the Markdown render

# Goal
The high review of `split_render_markdown` (pure move, 2026-10-29) found defects of the Markdown render that were
there before the split: inputs an LLM can produce give a broken list, an overwritten page or an inverted line
range, and the pages each carry their own copy of the list item and of the low confidence flag. No run has shown
them yet. Make the render robust to those inputs and give the pages one shared item format.

# Findings (code review of 2026-10-29, at 28bae6f)
* Multi-line descriptions: the domain page (`crates/retrodoc-render/src/markdown/domain.rs:31`) and the feature
  page (`feature.rs:41`) only `trim()` the description of a list item, while `agent_index.rs` collapses it with
  `one_line`. A description with a blank line ends the list item and starts a free paragraph; the pages and
  `llms.txt` then disagree.
* Path collisions: `segment` (`markdown/mod.rs:170`) folds case and maps every character outside `[a-z0-9_-]` to
  `-`, so two slugs such as `pay.invoice` and `pay-invoice` render to the same file; `render` returns two files
  with one path and the last one wins, with no warning. Not checked: whether the passes already make slugs
  distinct after this normalisation (`slugify` in `retrodoc-pipeline/src/features/mod.rs` normalises the feature
  slugs, use case slugs not checked).
* Inverted range: `use_case.rs:119` prints `path:9-3` when the end line is before the start line.
* Duplicated pieces: the item `- [name](link) (score): description` is written by three closures
  (`agent_index.rs:21`, `domain.rs:24`, `feature.rs:33`) that differ (`trim` against `one_line`, flagged score
  against `short_confidence`); the ⚠️ flag below `LOW_CONFIDENCE_THRESHOLD` is in `agent_index.rs:16` and in
  `score_line` of `mod.rs`.
* Minor: `render_with_docs` deep-clones every rule to drop those of vanished features (`mod.rs:103`), and
  `domain_page` filters the rules per feature again (`domain.rs:54`); `use_case.rs:99` writes
  `retrodoc_core::model::ActorKind::Human` in full.

# Approach
1. A test per edge case first (multi-line description, two slugs with one segment, inverted range), each failing
   on the current render.
2. One item helper in `markdown/mod.rs` (link, score with its flag, description on one line) used by the three
   pages; decide whether the domain and feature pages show the ⚠️ flag like `llms.txt`.
3. Path collisions: check first whether they can happen after the passes; if so, either disambiguate the segment
   (suffix) or report the collision; the choice is the user's.
4. The inverted range: print the start line only, or both in order.
5. The minor points when they fall in the code touched.

# Specs
* The rendered layout has no spec of its own; a change of the rendered bytes (flag on the pages, collapsed
  descriptions, suffixed paths) may need one (`skill:write-spec`).

# Resources
* `issues/done/split_render_markdown.md` (its `High review` section)
* `docs/crates/render-mcp-cli.md` (render behaviour)
* `crates/retrodoc-render/src/markdown/` and its `tests/`

# Hints
* A change of the rendered bytes rewrites the pages at the next `generate`: say so in the summary.
* `segment` is also a guard against path traversal (`hostile_slugs_cannot_escape_the_docs_dir`): keep it.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
Make the Markdown render robust to multi-line descriptions, inverted ranges and colliding paths, with one shared
list item for the three pages. Checked: the passes cannot produce colliding slugs (features and use cases through
`unique_slug`, domains merged by slug); a collision only comes from an edited or older artifact. Spec: `specs/markdown-render.md`.

## Plan
1. [x] Shared list item: one helper in `markdown/mod.rs` (link, score with its flag, description on one line) used by `agent_index.rs`, `domain.rs`, `feature.rs`; the flag kept in one place; verified by render tests through `retrodoc render` on edited artifacts; makes true MDR-1, MDR-2. (autonomous)
2. [x] Inverted range: `source_ref_label` puts the lower line first (the web UI shares it); `ActorKind` imported in `use_case.rs`; verified by a render test; makes true MDR-3. (autonomous)
3. [x] Path collisions: `render` fails naming the path when two files share one; the deep clone of the rules in `render_with_docs` replaced by references; `segment` guard traced; verified by render tests; makes true MDR-4, MDR-5. (autonomous)

4. [x] Review of the whole issue: findings of the `high` review fixed; verified by `just check`. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 2a172b9
- Flag on the pages: yes, ⚠️ before a weak score as in `llms.txt` (MDR-2).
- Inverted range: both lines, the lower first (MDR-3).
- Path collision: the render fails naming the path (MDR-4).

## Deliverable 1 summary
done: one `list_item` for the lists of `llms.txt`, the domain page and the feature page; the pages now flag a weak item with ⚠️ and collapse a multi-line name or description.
| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

red: MDR-1, MDR-2: the domain item read `Lists the orders.\n\nBy date.`; the domain item had no `(⚠️ 30%)`.
Code review: no findings
not verified: none.
decisions needed: none.
Follow-up: the per-feature rule filter of `domain_page` stays: `render_with_docs` filters the same rules per feature, but sharing it would only move the filter.

## Deliverable 2 summary
done: `source_ref_label` shows an inverted range with the lower line first (pages, rules, web UI); `ActorKind` imported in `use_case.rs`.
| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

red: MDR-3: the step showed `app/orders_controller.rb:9-3`.
Code review: no findings
not verified: the web UI page itself (it calls the same `source_ref_label`).
decisions needed: none.

## Deliverable 3 summary
done: `render` fails with `RenderError::PathCollision` naming the path when two items fold into one file; the rules are borrowed instead of deep-cloned; MDR-5 traced through the binary; spec `accepted`.
| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass, `specs/markdown-render.md` accepted |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

red: MDR-4: `render` succeeded on the domains `orders` and `Orders` (MDR-5 held already: the guard existed).
Code review: no findings
not verified: none.
decisions needed: none.

## Deliverable 4 summary (high review of 2a172b9..73a962c)
done: `SourceRef::lines` orders the cited lines once in core, read by the Markdown label, the MCP answers (`location`) and the web links (`code_href`); the rule tag uses the shared flag; the collision message no longer guesses the cause; the tests check the indented item of `llms.txt` and that a failed render leaves the pages untouched.
| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | high: 10 findings; low: see below |

red: MDR-3: the MCP `location` and the web link kept `9-3` (read from the review, no failing test written first: the core test was added with the fix).
High review: 10 findings. Fixed: web link order, MCP location order, the order put in `SourceRef` (altitude), the `item` helper of the tests, the collision message, the "writes no page" check, the flag copy of the rule tag. Not fixed: the collision as a hard error (decision 3 a of the user), paths that differ only by case (non-goal of the spec), the per-feature rule filter of `domain_page` (see the follow-up above).
Code review: no findings
not verified: the web UI in a browser.
decisions needed: none.
