---
priority: medium
why: 'every recall and precision rests on these references; the human spot check is due since the first benchmark'
---
# Have a human check the benchmark references

# Goal
The references of linkding and Lobsters (`reference.yaml`, `matches.yaml`, `entry-points.yaml`) were drafted by an
agent and checked against the code by a subagent; no human has read them in full. A wrong or missing reference item
moves recall and precision as much as a change of the pipeline. Expected: each reference read by the user once, the
items they reject or add fixed, and the reference marked with the date it was checked.

# Findings
* `issues/done/quality_benchmark.md`: "a human spot check of `matches.yaml` is still due".
* `issues/done/benchmark_passes_on_fixed_inputs.md` (2026-10-24): "Checked by an agent, not by a human: the human spot
  check stays due."
* `issues/done/second_application_in_benchmark.md`: the linkding reference (then 24 features, 29 rules) "not yet read
  in full by a human".
* Today: linkding 30 features (188 lines), Lobsters 38 features (274 lines).
* `PRODUCT.md`, risks: "the references are not all checked by a human".

# Approach
1. The agent prepares a short review sheet per application: each domain and feature of the reference with the code
   paths that back it, and the generated features of the latest run that match none (candidates the reference may
   miss).
2. The user reads it and marks keep, drop, rename or add; the agent applies only those marks.
3. Re-score the saved runs with `--saved-judge` (no LLM) and record how much the figures move.
4. A `checked_by_human: <date>` field (or a comment) in each reference, so the skill can say which figures rest on
   a checked reference.

# Milestone
Measured quality at the current code

# Resources
* `benchmark/linkding/`, `benchmark/lobsters/`, skill `quality-benchmark`.
* `crates/retrodoc-bench/src/benchmark/reference.rs` (format of the reference).

# Hints
* The user's time is the cost here: keep the sheet short, one line per item.
* Changing a reference makes the earlier figures incomparable: re-score the kept runs in the same deliverable.
