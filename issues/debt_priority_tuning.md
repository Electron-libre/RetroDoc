---
priority: medium
depends_on: [debt_priority_score]
why: 'the priority ranks the debt but a saved run shows it dominated by one kind; the milestone promises what to fix first'
---
# Tune the debt priority so that one kind or one large item does not fill the top of the list

# Goal
The debt page sorts the items by `weakness × (1 + ln(1 + commits)) × (1 + ln(1 + entry points))` (DBT-23 to DBT-26).
Every item without a score weighs 1, whatever its kind, and the commits are summed over the files of an item. On a
saved run the top of the list is filled by the set apart use cases, and large items outrank the small ones they hold.
Expected: the first items of the page are the ones a human should fix first, across the kinds.

# Findings
* 2026-11-04, lobsters, saved run `benchmark_baseline_at_head-dafea18-lobsters-1` (DeepSeek), served by `retrodoc ui`
  at commit 9a57835: 121 items, 106 of them set apart. The top 17 are set apart moderation use cases (255 to 687
  commits, 2 to 4 entry points, priority 13.7 to 17.2); the first low confidence item is 18th; the folders reached by
  no behaviour are last (0 entry points).
* The sum of the commits over the files of an item ranks a domain at or above each of its features (its files are
  the union of theirs), and a folder of many files left out by the budget can rank high on its commits alone (finding
  of the review of `issues/done/debt_priority_score.md`, not measured on a run).

# Approach
1. Measure first: dump the order of the items of a saved run or two (lobsters, linkding) with their parts, to see
   which kinds and which sizes fill the top.
2. Candidates, to compare on those runs:
   1. a weight per kind (a set apart use case below a contradicted or unbacked one), written in the spec;
   2. the max of the commits of the files of an item, or their mean, instead of their sum;
   3. both.

# Specs
* `specs/debt-view.md`: amends DBT-24 (weakness) and/or DBT-25 (commits).

# Milestone
Explore the documentation debt and audit it

# Resources
* `crates/retrodoc-mcp/src/debt/mod.rs` (`priority`, `Priority`), `specs/debt-view.md`.
* `issues/done/debt_priority_score.md` (its `# Tracking` and the review findings).

# Hints
* No LLM call is needed: the saved runs of the measures and `retrodoc ui` give the order.
