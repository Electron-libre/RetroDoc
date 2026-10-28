# Make the benchmark judge's empty feature answers visible and retried

# Goal

In some runs the benchmark judge proposes no feature pair at all, with nothing in the log, while the generated
features plainly cover the reference. The judged figures of the run then fall to the strict floor (often 0%),
and the mean of a series moves by tens of points for a reason that has nothing to do with the docs. A
measure should either get the judge's pairs or say that the run is unjudged.

# Findings

* 2026-10-28, `delivery_router`, deepseek-chat, `just benchmark --series hidden --runs 5 --no-confidence
  --domains-from surface` (`issues/done/surface_domains_default_decision.md`, deliverable 2): runs 2 and 5 have
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
* 2026-10-28, replay of the judge alone, 3 times on copies of `hidden-2` and `hidden-5` (ledger): on `hidden-2`
  every features answer (3/3) proposed pairs that were all dropped, because the judge wrote each name with its
  description as listed (`"Order clearing: Removes a customer's orders from the router."`); one domains answer
  of 3 lost a pair the same way. On `hidden-5` the 3 answers got pairs: its empty saved answer did not come back.

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

# Specs

* `specs/benchmark-scoring.md`: BSC-10 to BSC-14 (amended, `draft` until traced).

# Tracking

Plan: validated

## Understanding
A judge call that proposes no pair must be retried once, logged, and if still empty marked unjudged so the series
mean of the judged figures leaves it out instead of falling to the strict floor. Out of scope: re-judging or
rewriting saved `judge.yaml`.
Measure: the judge writes the names with their descriptions, so they are dropped; read them as the names first.

## Plan
1. [x] Measure: replay the features judge call alone on `clones-1793206893/hidden-2` and `hidden-5`, 3 times each,
   raw answer and dropped proposals kept (paid, under a cent); verified by the answers saved next to the clones and
   a ledger line; tells empty answer from names off.
2. [x] Names with their description: a proposal whose name is an offered name followed by `:` and more text is
   read as that name; verified by a `judge.rs` test; makes true BSC-14; `SCORING_VERSION` becomes 2. (~60 lines)
3. [x] Retry and warn: `propose_matches` asks once more when the answer proposes no pair, warns with the count of
   dropped proposals and when the second answer is still empty; verified by `judge.rs` tests on `FakeLlm`
   sequences; makes true BSC-10, BSC-11. (~150 lines)
4. [x] Unjudged runs: `Judgement` marks an unjudged kind (absent from the file when false), `RunReport` gets no
   judged figures for it, `benchmark-table` counts the unjudged runs per kind under the series; verified by `judge.rs` and `table.rs` tests; makes true BSC-12, BSC-13, spec back to `accepted`.
   (~300 lines)
5. [x] Fixes of the `high` review: a name cut at its colon only when the rest is the offered description (or its
   start), no warning for a repeated pair, `SCORING_VERSION` 2 doc names BSC-13; BSC-11 and BSC-14 reworded.

## Decisions
- Mode: autonomous
- Base commit: 8b25ad5f36e04e9b0a16675bdfe6b73cc32626f7
- Measure of deliverable 1 approved (paid, under a cent).
- Unjudged mark written in `judge.yaml` at the judge call; a `judge.yaml` saved before stays judged (not derived at scoring).

## Deliverable 2
- done: a proposal name that is an offered name followed by `:` is read as that name, pairs keep the offered
  names; `SCORING_VERSION` 2; the CLI test reads the constant instead of a literal.
- checks: `just check` pass; docs `docs/crates/pipeline.md`, `propose_matches` doc comment.
- red: BSC-14: `left: [] right: [Pair { generated: "Order clearing", reference: "Cancel a customer's order" }]`
- not verified: no new paid judge run on `hidden-2` with the fix.
- Code review: 1 finding; fixed: `offered_name` tried every prefix at once, so a shorter offered name could win
  over a longer one holding a `:`; now the whole name first, then the longest prefix (test added).

## Deliverable 3
- done: `propose_matches` asks once more when an answer leaves no pair (empty, unparseable, all dropped), warns
  with the count of dropped proposals and when the second answer has none either.
- checks: `just check` pass; docs `docs/crates/pipeline.md`, `propose_matches` doc comment.
- red: BSC-10, BSC-11: `left: [] right: [Pair { generated: "Billing", reference: "Payments" }]`; no `dropped=2`
  in the logs.
- not verified: no paid run.
- Code review: no findings

## Deliverable 4
- done: `Unjudged` (domains, features) in `judge.yaml` and `run-<n>.json`, absent when everything was judged; an
  unjudged kind has no judged recall and precision; `benchmark-table` counts the unjudged runs per kind under the
  series; spec `accepted`.
- checks: `just check` pass; docs `docs/crates/pipeline.md`, `docs/crates/render-mcp-cli.md`.
- red: BSC-12, BSC-13: `cannot find struct ... Unjudged`, `no field unjudged on Judgement`.
- not verified: no paid run; the printed `retrodoc benchmark` summary and the `pass_replay` tables still show an
  unjudged kind at the strict floor (only the log warns there).
- Code review: no findings

## Deliverable 5 (fixes of the `high` review, 10 findings)
- done: `offered_name` cuts a name at a `:` only when what follows is the offered item's description or its
  start (the replay showed verbatim copies), so a reference name holding a `:` that was not offered is no longer
  mapped onto another item; a repeated pair is not counted as dropped; `SCORING_VERSION` doc names both changes.
- checks: `just check` pass; spec BSC-11, BSC-14 reworded.
- red: BSC-14, BSC-11: `left: [Pair { generated: "Refund flow", reference: "Payments" }] right: []`; a WARN for a
  repeated pair.
- Code review (high): 10 findings: fixed the colon mapping, the repeat warning, the version doc; issue
  `issues/unjudged_kinds_in_replays_and_summary.md` for the replay tables and the printed summary (2 findings);
  not fixed: a genuine "no pair" twice is marked unjudged (decided in the spec, the table shows the count), the
  stacked retries (up to 4 calls on unreadable answers, cents), `judge` deriving the mark from empty pairs
  (documented on `propose_matches`), the listing format (a prompt change, unmeasured), the `harness.toml`
  source change (the user's, uncommitted).
- Code review (low): no findings
