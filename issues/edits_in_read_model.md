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
* 2026-11-05: `edits::record` takes the fingerprint (SED-8) and the cited hashes from the `GeneratedDocs` it is given,
  which must be the generated docs with no edit applied, while a review judges the edited ones: once the overlay is
  applied in the read model, keep the generated sections reachable as their own value (a distinct type, or the
  overlay dropped inside `record`) so the form can't hand the edited docs to it (high review of `edit_store`).

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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
Apply the edits of `.retrodoc/edits.json` over the domains, features and use cases for every reader: the read model
(pages, MCP answers, search), `retrodoc ui` and `retrodoc mcp` live, and the Markdown of `render`/`generate`. Out of
scope: the edit form, the mark "edited by a human" and the outdated edit (`web_edit_form`, `edit_outdated`).
Means: the render requirements go to `specs/section-edit.md` (SED-30 to SED-32), `markdown-render.md` covering only
the edge cases of the layout.

## Plan
1. [ ] Overlay in the read model: a pure `edits::apply` over domains, features and use cases; `GeneratedDocs` keeps
   the generated sections as their own value beside the shown ones, `load` applies the saved edits (warns of a file
   it can't read), the search index is built over the shown sections, `edits::record` fingerprints the generated
   sections, a review the shown ones; verified by read model, search and MCP tool tests; makes true SED-23 to SED-26,
   SED-29 (load), MCP-14, DSR-6. (~450 lines)
2. [x] `publish` applies the same edits before the report and the render, fails on a file it can't read; verified by
   a CLI test of `render` with an edit, twice, and with a broken file; makes true SED-30, SED-31, SED-32. (~200 lines) (autonomous)
3. [ ] Live edits: `retrodoc mcp` re-reads `edits.json` at each tool call and `retrodoc ui` at each request,
   re-applying the overlay (and rebuilding the site and the index) only when its content changed; verified by
   server tests saving an edit between two requests; makes true SED-27, SED-28, SED-29. (~400 lines)

## Decisions
- Mode: autonomous
- Base commit: ea1ea39e363460cf5c03a75cca07d590ac5c18a2
- `render`/`generate` fail on an `edits.json` they can't read, writing no doc (SED-32); `ui`/`mcp` warn and show the
  generated text (SED-29).
- No mark "edited by a human" in the Markdown in this issue: a follow-up issue tracks it.
- `ui`/`mcp` tell a change of `edits.json` by comparing its content at each request.

## Deliverable 1: overlay in the read model

done: `edits::apply` lays the edits over `Sections`; `GeneratedDocs` shows them (pages, MCP answers, search) and
keeps `generated()`, which `edits::record` fingerprints; `load` applies `.retrodoc/edits.json`, warns and shows the
generated text when it can't be read.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (SED, MCP, DSR `draft` until their last requirement is traced) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

red: SED-25: second edit fingerprint `6f46…` ≠ first `05bd…` with `record` on the shown sections; DSR-6: search
`dispatch courier` gave `[]`; MCP-14: `list_domains` lacked `Invoicing` (the overlay left out of `load`).

## Deliverable 2: `publish` applies the edits

done: `render` and `generate` lay the edits over the domains, features and use cases they render (and over the
features and use cases of the debt report; the reconciliation keeps the generated text its verdicts judged); an
`edits.json` that can't be read fails them before any doc is written.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | low, clean |

red: SED-30, SED-32: the domain page kept "Orders of the shop." (the edit was not rendered); SED-31 passed before
the code (the render was already idempotent, the test guards it with edits).

Code review: no findings

not verified: `generate` failing at its render step on a broken `edits.json` (same `publish`, tested through
`render`). decisions needed: none.
