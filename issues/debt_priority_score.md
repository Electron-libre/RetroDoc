---
priority: medium
depends_on: [web_debt_view_of_the_docs, web_debt_view_of_the_code]
why: 'says what to fix first; needs every kind of debt item to rank'
---
# Rank the debt items by a priority that crosses the debt with the git churn and the entry points

# Goal
The debt view lists what needs a human but not what to fix first. Expected: each debt item gets a priority score that
crosses how weak it is with how much its code changes (git churn) and how many entry points it touches, and the debt
view sorts by it, with the parts of the score shown.

# Findings
* The churn per file is collected at the scan (`FileHistory::commit_count`, `crates/retrodoc-ingest/src/git_history.rs`,
  one revwalk) and used only to rank the files of the budget (`crates/retrodoc-pipeline/src/ranking.rs`); it is not
  saved in `.retrodoc/cache/`.

# Approach
1. Where the churn comes from: (a) read the git history when `retrodoc ui` opens (one revwalk, time to measure on a
   large repository), or (b) save it at `generate` in a cache artifact (no git at read time, but a new artifact).
2. The formula: candidates are a product of normalized factors like the ranking's (`weight × (1 + ln(1 + commits))`),
   or a sum of ranks; compare the order they give on a saved run before choosing.

# Specs
* The spec of the debt view.

# Milestone
Explore the documentation debt and audit it

# Resources
* `crates/retrodoc-pipeline/src/ranking.rs`, `crates/retrodoc-ingest/src/git_history.rs`.

# Hints
* A choice of (b) adds an artifact: `docs/ARCHITECTURE.md` §7 and the fingerprints of the rerun (`specs/rerun.md`).

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
Each debt item of `/debt` gets a priority `weakness × (1 + ln(1 + commits)) × (1 + ln(1 + entry points))`, the list
is sorted by it and each row shows the three parts. Churn read once when `retrodoc ui` starts (one revwalk: 1.5 s on
lobsters, 4,823 commits; 0.6 s on linkding), no new artifact. Out of scope: weights per kind, the MCP server.

## Plan
1. [ ] Read model: the priority and its parts on each debt item, the debt list sorted by it, the churn given to the
   read model; verified by tests of `retrodoc-mcp/src/debt`; makes true DBT-8, DBT-23, DBT-24, DBT-25, DBT-26.
2. [ ] Web and CLI: `retrodoc ui` reads the history at start, each debt row shows the priority and its parts, the
   page says when the commits are unknown; verified by tests of `retrodoc-web`; makes true DBT-27, DBT-28 (spec
   `accepted`).

## Decisions
- Mode: autonomous
- Base commit: b196eba0fb345363f978cc00f35fc3f6dbd83fe2
- Churn read when `retrodoc ui` starts, no artifact (1 a); the product formula, not measured against a sum of ranks
  (2 a); every item without a score weighs 1 (3 a).

## Deliverable 1
- done: each debt item of the read model carries its `Priority` (weakness, commits, entry points), the list is sorted
  by it, the churn per file set by `GeneratedDocs::with_churn`.
- red: DBT-8, DBT-23, DBT-24, DBT-25, DBT-26: `cannot find type Priority`, `no method named with_churn`.
- Code review: pending
- not verified: the order on a saved run (the churn is only read by `retrodoc ui` in deliverable 2).
