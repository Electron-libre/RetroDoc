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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
The audit page gains the trends of the saved snapshots: a table of the figures and one of the debt per kind, newest
first, the worse figures marked, and one server-drawn SVG line chart per score; fewer than two snapshots says so.
Out of scope: trends of the stale share, the contexts and the cost (the snapshot lacks them or they don't compare).

## Plan
1. [x] Trend tables: the rows of the snapshots in `retrodoc-web` (`trends.rs`), the two tables, the worse marks and
   the line of fewer than two runs on `/audit`, texts en/fr; verified by `tests/audit_page.rs` over saved snapshots;
   makes true AUD-11, AUD-12, AUD-13, AUD-15. (autonomous)
2. [ ] Trend charts: one SVG line chart per score, built from the same rows (skill `dataviz`), checked headless in
   Chrome; verified by `tests/audit_page.rs` (points, broken line, no chart for a score no run has); makes true AUD-14,
   spec `accepted`.

## Decisions
- Mode: autonomous
- Base commit: e8acb9573f0111efbc2dba7d2a938c41affa4675
- A figure is marked worse as soon as its shown (rounded) value is worse than the run before it, no threshold (1 a).
- No chart of the debt: the table per kind carries it (2 a).
- With one snapshot, only the line saying the trends need two runs (3 a).
- The runs are evenly spaced on the charts, not by date.

## Summaries

### 1. Trend tables
- done: `/audit` shows the trends of the saved runs (figures table, debt-per-kind table, `▼` marks, line under two runs), en/fr.
- red: AUD-11, AUD-12, AUD-13, AUD-15: `<table class="trends"` not in the page (panicked in `between`).
- Code review: no findings
- not verified: the page in a browser (with deliverable 2).
