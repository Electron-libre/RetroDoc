# Split the Markdown render into its tests and one file per page

# Goal
`crates/retrodoc-render/src/markdown.rs` holds every page of the generated docs and their tests in one file,
long to read and to review. Split it along the seams it already has, with no behaviour change: `just check`
and a second `generate` writing nothing stay green.

# Findings (exploration of 2026-10-29, at 2a9c27a)
* Cut from `issues/split_largest_files.md` (one issue per crate, to stay under the size limit).
* `crates/retrodoc-render/src/markdown.rs`, 1,122 lines: entry points `render`, `render_with_rules`,
  `render_with_docs` at 42-159; paths and links at 160-248; one function per page (`agent_index` 249,
  `domain_page` 326, `feature_page` 377, `use_case_page` 442, `step_item` 529); tests at 565-1122 (about 556
  lines: fixtures, pages and layout, agent index, rules and doc links).

# Approach
1. Tests apart: `markdown.rs` becomes `markdown/mod.rs` (a `git mv`) plus `markdown/tests/` grouped by topic
   (about 1,110 changed lines, the tests being dedented: two deliverables, the pages and layout then the rules
   and doc links).
2. One page per file: `markdown/<page>.rs` for the agent index, the domain, the feature and the use case pages
   (about 660 changed lines).
3. One commit per deliverable.

# Resources
* `issues/split_largest_files.md` (the issue it was cut from, and its plan)
* `issues/done/refactor_code_hygiene.md` (how the earlier splits were made)
* `docs/crates/render-mcp-cli.md` (module list to update)

# Hints
* after split_mcp_tools
* Keep the public API of the crate (re-export from `mod.rs`); `feature_path`, `use_case_path` and `link` are
  `pub(crate)` and used by `rules.rs` and `doc_links.rs`.
* The line numbers above move with every commit; read the file again at planning.
