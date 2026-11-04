---
priority: medium
depends_on: [web_debt_view_of_the_docs]
why: 'tells where the debt concentrates; reads the debt items of the view'
---
# Show an audit dashboard of the docs in the web interface

# Goal
Deciding where to improve the docs needs to see where the debt concentrates. Expected: an audit page of `retrodoc ui`
with the figures of the run (overall confidence, business language, coverage of the entry points, share of stale
sections, cost per pass, progress of the bounded contexts) and a heatmap of the domains by indicator, each cell
linking to the debt view filtered on that domain and kind.

# Findings
* `DebtReport` holds the overall confidence and business language, the per-domain confidence (`DomainDebt`) and the
  entry points per context and per feature.
* The cost per pass is in `.retrodoc/cache/usage.json` (`crates/retrodoc-pipeline/src/usage_log.rs`).
* The progress of the bounded contexts (contexts done and `pending`) is computed in
  `crates/retrodoc-pipeline/src/contexts/run.rs`; it is not saved as such.

# Approach
1. The figures from the debt report, `usage.json` and the contexts, in the read model.
2. The heatmap as an HTML table with a colour scale (no chart library: the assets are vendored, ADR 0039); its
   indicators the kinds of the debt view.
3. A run with no confidence or no contexts shows the figures it has and says which are missing.

# Specs
* The spec of the debt view, or `specs/web-ui.md`.

# Milestone
Explore the documentation debt and audit it

# Resources
* `crates/retrodoc-pipeline/src/report/`, `usage_log.rs`, `contexts/run.rs`.
* `docs/adr/0039-a-server-rendered-web-interface-over-the-generated-docs.md`.

# Hints
* Read the dataviz rules before choosing the colours of the heatmap; it must read in light and dark themes.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
A page `/audit` of `retrodoc ui` with the figures of the run (confidence, business language, entry point coverage,
stale share, tokens and time per pass, contexts done and pending) and a heatmap domain × debt kind linking to `/debt`
(`specs/audit-dashboard.md`). Out of scope: the code kinds (`web_debt_view_of_the_code`), a money cost (no prices in
the code). Means differ: the contexts progress is read as `fingerprints.json` records it, not re-judged.

## Plan
1. [x] Audit figures in the read model: `retrodoc_mcp::audit` builds the figures (report overall scores, behaviour
   coverage, stale share, last `generate` of `usage.json`, contexts from `contexts.yaml` and `fingerprints.json`) and
   the heatmap cells with their levels, independent of HTTP; verified by internal tests; makes true nothing alone
   (prepares AUD-1 to AUD-8). ~450 lines. (autonomous)
2. [ ] The page `/audit`: route, template, heatmap table and its shades in the stylesheet, missing figure lines,
   header link, English and French texts; verified by router tests and a check in headless Chrome; makes true AUD-1
   to AUD-10. ~600 lines.

## Decisions
- Mode: autonomous
- Base commit: c430e3be271e6df382f96aca30e78e447f983e60
- 1 a: the contexts progress as `fingerprints.json` records it, not re-judged.
- 2 a: the cost of the last `generate` of `usage.json`.
- 3 a: 4 levels by count over the largest cell of the table.
- 4 a: gold shades of the dark charter, the count always written.
- 5 a: no "Start here" line; the header link only.

## Deliverable summaries

### 1. Audit figures in the read model
done: `retrodoc_mcp::audit::audit` gives the overall scores, the entry point coverage, the stale share, the last
`generate` of the usage history, the contexts progress (`retrodoc_pipeline::recorded_progress`) and the heatmap rows
with their levels; `GeneratedDocs::repo_root`.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (`audit-dashboard` draft) |
| docs | `docs/crates/render-mcp-cli.md`, `docs/crates/pipeline.md` |
| code review | no findings |

red: AUD-5: none (the test of `recorded_progress` was written after the function); AUD-1, AUD-3, AUD-6, AUD-7: the
heatmap test failed on the expected row (`assertion left == right failed`), a use case low in confidence being also
set apart: the expectation was wrong, not the code.

Code review: no findings

not verified: none. decisions needed: none.
