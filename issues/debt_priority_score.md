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
