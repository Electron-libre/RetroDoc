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
