# Make the benchmark judge's empty feature answers visible and retried

# Goal

In some runs the benchmark judge proposes no feature pair at all, with nothing in the log, while the generated
features plainly cover the reference. The judged figures of the run then fall to the strict floor (often 0%),
and the mean of a series moves by tens of points for a reason that has nothing to do with the docs. A
measure should either get the judge's pairs or say that the run is unjudged.

# Findings

* 2026-10-28, `delivery_router`, deepseek-chat, `just benchmark --series hidden --runs 5 --no-confidence
  --domains-from surface` (`issues/surface_domains_default_decision.md`, deliverable 2): runs 2 and 5 have
  `features: []` in `.retrodoc/benchmark/judge.yaml` while their domains got pairs in the same judgement. Run 2's
  five features ("Order placement and rider assignment", "Delivery time computation", "Order clearing",
  "Rider routing", "Location measurement") cover 7 or 8 of the 8 reference features by hand. The series reads
  57% (0-100) judged feature recall; with the two runs audited by hand it is about 95%.
* The same thing happened in `issues/done/brief_injection_experiments.md`: runs 2 and 3 of the `no brief`
  features replay proposed no pair (judged = strict, 17% and 21%, against 67% for run 1).
* No warning in the run logs: `complete_json` warns when an answer is unparseable twice, so the answer was
  probably parsed. Either it was an empty `matches` list, or every proposal was dropped by
  `propose_matches` because a name did not match an offered item, which is only logged at `debug`.
  Not measured which of the two.

# Approach

1. Measure first: replay the judge alone on the saved runs (`clones-<timestamp>/hidden-2`, `hidden-5`) a few
   times, logging the raw answer and the dropped proposals, to see whether the answer is empty or its names are
   off.
2. Candidates (can be combined):
   1. Log the dropped proposals at `warn` with a count, and warn when a judgement of features proposes no pair
      while there are generated and unmatched reference features.
   2. Ask again once when the features answer is empty or all of its proposals are dropped.
   3. Match the names of the proposals more loosely (case, punctuation, a name with its description).
   4. Mark the run as unjudged in `run-<n>.json` and leave it out of the mean of the judged figures, with
      the count of unjudged runs in the table.

# Resources

* `crates/retrodoc-pipeline/src/benchmark/judge.rs` (`propose_matches`, `MATCH_SYSTEM_PROMPT`),
  `crates/retrodoc-pipeline/src/response.rs` (`complete_json`), `crates/retrodoc-pipeline/src/benchmark/table.rs`.
* Skill `quality-benchmark`; `issues/done/benchmark_spread_and_judge_bias.md` (the judge's known biases).

# Hints

* The saved judgements are what the ledger's re-scores read (`judge.yaml`): a change of the judge must not
  rewrite the ones already saved.
* Every judge call is paid: the replay of step 1 costs cents on `delivery_router`, ask before running it.
