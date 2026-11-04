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
1. [x] Read model: the priority and its parts on each debt item, the debt list sorted by it, the churn given to the
   read model; verified by tests of `retrodoc-mcp/src/debt`; makes true DBT-8, DBT-23, DBT-24, DBT-25, DBT-26.
   (autonomous)
2. [x] Web and CLI: `retrodoc ui` reads the history at start, each debt row shows the priority and its parts, the
   page says when the commits are unknown; verified by tests of `retrodoc-web`; makes true DBT-27, DBT-28 (spec
   `accepted`). (autonomous)
3. [x] Fixes of the `high` review of the whole issue; verified by tests of `retrodoc-mcp/src/debt` and
   `retrodoc-web`; makes true DBT-25, DBT-27. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: b196eba0fb345363f978cc00f35fc3f6dbd83fe2
- Churn read when `retrodoc ui` starts, no artifact (1 a); the product formula, not measured against a sum of ranks
  (2 a); every item without a score weighs 1 (3 a).

## Deliverable 1
- done: each debt item of the read model carries its `Priority` (weakness, commits, entry points), the list is sorted
  by it, the churn per file set by `GeneratedDocs::with_churn`.
- red: DBT-8, DBT-23, DBT-24, DBT-25, DBT-26: `cannot find type Priority`, `no method named with_churn`.
- Code review: 1 findings
  - a rule matched by its slug alone could take the files of a same-named rule of another feature: not fixed, a rule
    slug is `<feature-slug>-<n>` and unique in the rules of a run, and a rule whose feature left the run (`?` in the
    label) keeps its code evidence.
- not verified: the order on a saved run (the churn is only read by `retrodoc ui` in deliverable 2).

## Deliverable 2
- done: `retrodoc ui` reads the churn at start (`GeneratedDocs::with_history`), each debt row shows its priority and
  its parts, the page says when the git history could not be read; spec `accepted`.
- red: DBT-25, DBT-27, DBT-8, DBT-28: `no associated function named with_history`.
- Code review: no findings
- Saved lobsters run (`benchmark_baseline_at_head-dafea18-lobsters-1`, 121 items, 106 set apart): the top 17 are set
  apart moderation use cases (255 to 687 commits, 2 to 4 entry points, priority 13.7 to 17.2); the first low
  confidence item is 18th, the unbacked narrative within the top 30, the folders reached by no behaviour last (0 entry
  points).

## Deliverable 3 (high review)
- done: an item whose feature left the run counts 0 commits (doc items and sections, the file lookup shared by
  `section_parts`), zero commits and entry points take the singular in French, a log line before the history walk of
  `retrodoc ui`, the README sentence reordered.
- red: DBT-25: `SetApart ?/gone/lost` counted 9 commits; DBT-27: `0 commits, 0 points d'entrée` in French.
- High review: 9 findings
  - French plural of zero: fixed.
  - the history walk blocks the start of `retrodoc ui`: a log line added; the walk at start is decision 1 a (1.5 s on
    lobsters).
  - an unborn `HEAD` gives an empty churn and no note: not fixed, the history is readable and every file has 0 commits.
  - the sum over files favours large items (a domain over its features, a big folder left out): not fixed, the
    formula of DBT-25 validated; a decision for the user (see the lobsters run of deliverable 2).
  - an item whose feature left the run counted commits: fixed.
  - linear lookups per item and `value()` recomputed in the sort: not fixed, a few million comparisons at most per
    request on the largest runs measured.
  - the file lookup of feature and use-case doc items duplicated `section_parts`: fixed; the rule evidence filter
    shared with `cited_paths` left as is (one line).
  - the `git` test helper duplicated across test crates: not fixed, no shared test crate.
  - the README clause misplaced: fixed.
- Code review: no findings
