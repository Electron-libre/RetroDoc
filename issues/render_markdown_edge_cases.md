---
rank: 1400
why: 'low (high review of `split_render_markdown`, 2026-10-29): edge cases of the render no run has shown yet; next to `mcp_tools_answer_edge_cases`, same kind, before `git_log_follow_renames`'
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
