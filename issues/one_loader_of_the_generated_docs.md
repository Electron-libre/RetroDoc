---
priority: medium
why: 'three loaders of the same docs already diverge on the edits; every later refactoring of the readers starts from one'
---
# Load the generated docs in one place for every reader

# Goal
The docs a `generate` saved are assembled three times, each with its own choice of artifacts, edits and debt: the
read model of the MCP server, the web interface and `ask`, the `publish` of `render` and `generate`, and the
`report` command. Expected: one function that loads the docs as shown (generated sections, edits, rules, debt,
coverage), used by the three, so a reader can no longer see other docs than another.

# Findings
* 2026-11-07: `GeneratedDocs::load` (`crates/retrodoc-mcp/src/read_model.rs`) loads the features, use cases, domain
  map, glossary, rules, scope and behaviour coverage, then applies the edits.
* 2026-11-07: `publish` (`crates/retrodoc-cli/src/commands/docs.rs`) loads the features, use cases and domain map
  again, applies the edits, then builds the debt report with `build_report` on the edited sections, plus scope,
  behaviour coverage, technical features and `add_doc_debt`.
* 2026-11-07: `report` (`crates/retrodoc-cli/src/commands/report.rs`) repeats the same report assembly on the
  sections with no edit applied, then loads `GeneratedDocs` a second time for the sections marked wrong. With edits
  saved, the printed report and `docs/_retrodoc/coverage-report.md` can differ; no requirement of
  `specs/section-edit.md` says which is meant.
* The assembly of the debt report (`add_doc_debt` and its helpers) lives in the CLI crate, out of reach of the web
  interface and the MCP server.

# Approach
1. Decide with the user whether `report` shows the edited sections, as the published report does (amend
   `specs/section-edit.md` if so).
2. Move the assembly of the debt report out of the CLI, next to the read model, and have `publish`, `report` and
   `GeneratedDocs::load` take the loaded docs from one function.
3. Tests: the three readers give the same sections and the same debt on a saved run with edits.

# Specs
* `specs/section-edit.md` (the report and the edits, if step 1 says so), `specs/markdown-render.md`.

# Milestone
The readers of the docs out of the pipeline

# Resources
* `docs/crates/render-mcp-cli.md`, ADR 0045, ADR 0016.
* `issues/done/edits_in_read_model.md` (why `publish` does not go through the read model today).

# Hints
* The edits need the generated sections as their own value (`edits::record` fingerprints them): keep them reachable.
* The rendered Markdown must stay byte-identical on an unchanged run (rerun no-op).
