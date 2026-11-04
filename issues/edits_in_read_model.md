---
priority: medium
depends_on: [edit_store]
why: 'an edit pays off only when every reader of the docs sees it'
---
# Show the human edits wherever the docs are read

# Goal
The pages, the MCP answers, the search and the Markdown written by `render` and `generate` read the generated
sections only. Expected: an edited field replaces the generated one for every reader, the generated text kept
beneath, so the docs a team corrected say the same thing in the browser, to an agent and in the docs folder.

# Findings
* 2026-11-05: `GeneratedDocs::load` (`crates/retrodoc-mcp/src/read_model.rs`) reads the artifacts of the cache and
  builds the search index once, in `GeneratedDocs::new`; `retrodoc ui` and `retrodoc mcp` load it once at start.
* 2026-11-05: `render` and `generate` write the Markdown from the artifacts (`publish` in
  `crates/retrodoc-cli/src/commands/docs.rs`), not through the read model.

# Approach
1. Apply the edits over the domains, features and use cases when the docs are loaded, keeping the generated
   section for whoever needs it (the page of `web_edit_form`, the comparison of `edit_outdated`).
2. Reload them, and the search index, after an edit saved while the server runs (`retrodoc ui`), and at each answer
   of `retrodoc mcp` or when the file changed.
3. `publish` applies the same edits before rendering, so the rendered Markdown holds them and stays deterministic.
4. A review fingerprints the section as shown: an edit makes a former review outdated (decided with the user on
   2026-11-05; REV-20 already does it once the edits are applied).

# Specs
* `specs/section-edit.md`: where the edits apply.
* `specs/markdown-render.md`, `specs/mcp-server.md`, `specs/docs-search.md`: the edited text in their output.

# Milestone
Edit the docs

# Resources
* `crates/retrodoc-mcp/src/read_model.rs`, `crates/retrodoc-cli/src/commands/docs.rs`, `crates/retrodoc-render/`.

# Hints
* The rendered Markdown must stay idempotent: the same artifacts and edits write the same bytes.
