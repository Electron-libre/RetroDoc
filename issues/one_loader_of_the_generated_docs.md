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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-19

## Understanding
One assembly of the docs as shown (sections with edits, rules, debt report with reconciliation, scope, coverage,
technical features, set aside, marked wrong) in `retrodoc-mcp`, used by `publish`, `report` and the debt of the read
model. Found while reading: the edits change no text of the report today (it shows slugs, and the generated names in
the reconciliation lists); the printed and the published reports differ by "Sections marked wrong" (DBT-35, printed
only), and the web debt has a fourth assembly (`debt::docs_report`, with no domain map). Out of scope: the
`doc_links` of the pages (they need `retrodoc-render`), new verdicts of the reconciliation on edited text.

## Plan
1. [x] ~~Move `add_doc_debt`, `project_docs` and `without_generated` to `retrodoc-mcp`~~: dropped at the gather of
   deliverable 2, nothing to move: the project docs stay read by the CLI (`project_docs` needs the config and
   `anyhow`), and `add_doc_debt` is replaced by the assembly in `retrodoc-mcp`.
2. [x] One loader: `GeneratedDocs` loads strictly for `publish` (an unreadable `edits.json` fails, SED-32) and builds
   the one debt report of the docs as shown (edited names on the reconciliation items, marked wrong included);
   `publish`, `report` and `debt::docs_report` take it; verified by a CLI test of the scenario of SED-39/SED-40
   (render then report on a run with an edit and a review) and the existing report, render and debt tests; makes
   true SED-39, SED-40, SED-H1. (~500 lines) (autonomous)
3. [ ] Docs of the whole issue (`docs/crates/render-mcp-cli.md`, ADR 0045 consequences) and the fixes of its `high`
   review.

## Decisions
- Mode: autonomous
- Base commit: 201c4c563a51cab06d770e665c4f8e13eb5cb30a
- The report names an edited section as its edit gives it, the verdicts of the reconciliation kept as given on the
  generated text (SED-40).
- `coverage-report.md` lists the sections marked wrong as `report` does: the two are one text (SED-39).
- The debt of the read model (`debt::docs_report`, the web debt view) takes the same report, in deliverable 2.

## Deliverable 2: one loader

done: `GeneratedDocs::debt_report` (`retrodoc-mcp/src/report.rs`) assembles the debt report of the docs as shown;
`publish` (through `GeneratedDocs::load_with_edits`, the edits loaded strictly first) and `report` both take it from
`commands::docs::debt_report`, and `debt::docs_report` reads the domain map of the run; `add_doc_debt` and the
second load of `report` are gone.

| check | result |
|---|---|
| tests | `cargo test --workspace` green (64 suites) |
| lint | `cardinal run project-check` green |
| specs | SED-39, SED-40, SED-H1 traced; `specs/section-edit.md` accepted, split into Behaviour and Contract |
| docs | `docs/crates/render-mcp-cli.md`, `docs/crates/pipeline.md`, `docs/tutorial.md` |
| code review | see below |

red: SED-H1, SED-39, SED-40: `assertion left == right failed`: the printed report held "Sections marked wrong", the
published one did not.

Code review: no findings

not verified: a `generate` on a real repository (the rendered `coverage-report.md` changes once on a repo with
reviews marked wrong or edited names, then stays byte-identical).

decisions needed: none.
