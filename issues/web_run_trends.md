---
priority: medium
depends_on: [run_snapshot_at_generate]
why: 'the dashboard half of the milestone; useful from the second snapshot on'
---
# Show the trends of the figures across runs on the audit page

# Goal
`/audit` shows the figures of the last run only. Expected: the audit page shows how the overall confidence, the
business language, the entry point coverage and the debt per kind moved across the saved snapshots, so whoever
audits the docs sees whether they get better and which run made them worse.

# Findings
* 2026-11-04: `/audit` (`specs/audit-dashboard.md`, AUD-1 to AUD-10) holds the overall scores, the coverage, the stale
  share, the cost of the last `generate`, the progress of the contexts and the heatmap of the debt, all of the last
  run.

# Approach
1. A table with one row per snapshot (date, commit, the figures), newest first.
2. A small line chart per score over the runs, drawn on the server like the other pages (no client script), the
   table staying the carrier of the figures.
3. A repository with one snapshot or none says the trends need two runs.

# Specs
* `specs/audit-dashboard.md`: new requirements of the trends.

# Milestone
Run history

# Resources
* `crates/retrodoc-web/src/` (the audit page and its templates), `crates/retrodoc-mcp/src/audit`.
* The spec of the run history written by `issues/done/run_snapshot_at_generate.md`.
* Skill `dataviz` before drawing the chart; the browsers of the machine to check the page headless.
