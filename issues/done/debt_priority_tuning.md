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
* 2026-11-04, measured at commit dadbe3b on four saved runs of dafea18 (lobsters and linkding, hidden-1 and shown-1),
  each item's files and their commits dumped by a throwaway patch, then ranked under the candidates (no LLM call):
  * hidden runs (no collected doc): set apart use cases fill 17 to 19 of the top 20 whatever the commits
    aggregation; a weight 0.5 or less on the kinds left out by design puts the low confidence and unbacked items first.
  * shown runs: undocumented items are 291 of 401 (lobsters) and 80 of 131 (linkding); the top 20 is undocumented
    (lobsters, no contradiction found) or undocumented and contradicted (linkding).
  * the max of the commits instead of their sum sends the big folders down (linkding `bookmarks/migrations`, 55
    files: rank 17 to 34; lobsters 150 files: 59 to 118); the mean does the same, and sinks an item holding one hot
    file among cold ones.
  * a domain outranks its features through its entry points (the sum of its features': lobsters `story-publishing`
    117), not its commits: first under the sum, the max and the mean.

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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
Rank the debt so that the items whose doc may be false come first and a large folder does not climb on its size: a
weight per kind in three tiers (DBT-29) and the most commits of one file instead of their sum (DBT-25). Out of
scope: folding the items a larger one holds (a domain stays above its features, a non-goal of the spec).

## Plan
1. [x] Weight per kind and max of the commits: `Priority` gains the weight, the commits are the max over the files,
   the debt row shows the weight; verified by tests of `retrodoc-mcp/src/debt` and `retrodoc-web`, and the order of
   the four saved runs re-dumped; makes true DBT-23, DBT-25, DBT-27, DBT-29 (spec `accepted`). (autonomous)
2. [x] Fixes of the `high` review of the whole issue; verified by tests of `retrodoc-mcp/src/debt`; makes true DBT-25,
   DBT-29. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: dadbe3b34e5c8a8a5f248b198e6e75f90660aacc
- Weights per tier 1 / 0.5 / 0.25 (1 a); the max of the commits of the files of an item (2 a); a domain stays
  above its features, no folding (3 a).

## Deliverable 1
- done: `Priority` gains the weight of its kind (`DebtKind::weight`, three tiers), the commits are those of the
  busiest file of an item, each debt row shows the weight (English and French); spec `accepted`.
- red: DBT-23, DBT-25, DBT-29: `no field weight on Priority`, `no method named weight`; DBT-25: the commits of
  `billing/pay/refund` 110 (sum) against 100 expected.
- Order of the four saved runs at this code (`retrodoc ui`, `/debt`): lobsters hidden, low confidence 1st and the
  unbacked narrative 2nd (were 19th and 20th); linkding hidden, low confidence 1st (was 20th); lobsters shown, top
  20 undocumented (no contradiction in the run); linkding shown, the 3 contradicted first, then undocumented; the
  folders reached by no behaviour from rank 59 to 70 (lobsters hidden), the set apart items from 37 to 215
  (lobsters shown).
- Code review: no findings
- not verified: nothing left.

## Deliverable 2 (high review)
- done: the DBT-29 scenario rewritten on a reachable score (0.4: low confidence and set apart for its confidence),
  the DBT-25 scenario of a folder of 10, 4 and 0 commits tested, the doc of `DebtList::items` says the order by
  priority.
- red: none (the folder fixture changed under green code: the max was already right).
- High review: 6 findings
  - the DBT-29 scenario at 0.6 cannot happen (threshold 0.5): fixed.
  - the DBT-25 scenario 10, 4, 0 untested: fixed.
  - one hot shared file (routes, schema) gives every item citing it the same commits: not fixed, the max is decision
    2 a, measured on four runs; the sum ranked items by their size instead.
  - `value()` recomputed in the sort: not fixed, as in `issues/done/debt_priority_score.md` (a few thousand items).
  - `Priority::weight` derived from the kind, its default 1: not fixed, `debt_of` sets the priority of every item
    before it leaves the list; the weight stays in `Priority` so `value()` and the row read one struct.
  - the doc of `DebtList::items` said the order by kind: fixed.
- Code review: no findings
