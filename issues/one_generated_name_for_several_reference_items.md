# Let a generated name stand for several reference items in the benchmark

# Goal
A generated domain or feature can only stand for one reference item, so the judged recall under-counts on
repositories where the model merges two capabilities into one feature. Count a merged feature for each reference
item it really holds, so the recall of an application does not depend on how the model groups capabilities.

# Findings (code review `high` of `second_application_in_benchmark`, 2026-10-27, Lobsters, deepseek-chat)
* `matching::score` keeps the first hand-written pair of a generated name; a second pair of the same name is
  ignored, so `matches.yaml` cannot say it either.
* `judge` only reads the generated and reference names left unmatched (`judge.rs`, `unmatched_generated`,
  `unmatched_reference`), and proposes one reference item per generated name.
* On the complete run of Lobsters, "Voting and flagging" and "Story voting, flagging, hiding and saving" were paired
  with "Upvote stories and comments" only; "Flag stories and comments" and "Hide and save stories" stayed
  uncovered although these features hold them.

# Approach
1. Measure first, without LLM: on the saved `judge.yaml` of the lobsters run and of a linkding run, count by hand
   the generated features that hold more than one reference feature, and the recall they would add.
2. If the gain is above the spread between runs: let a pair list several reference items (`matches.yaml`), and
   let the judge propose several for one generated name; precision keeps counting the generated name once.
3. Otherwise, close it with the count, and say in skill `quality-benchmark` that the recall is a floor when the
   model merges features.

# Resources
* `crates/retrodoc-pipeline/src/benchmark/matching.rs` (`score`), `judge.rs`
* `benchmark/lobsters/matches.yaml` (header: superset pairs), skill `quality-benchmark`
* The ledger of measures: the lobsters run `second_application_in_benchmark-d8d8c92-1` keeps its `judge.yaml`

# Hints
* A change of the scoring makes the earlier figures of the ledger incomparable: rerun no paid measure for it,
  re-score the saved runs.
