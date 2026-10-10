# Check use case steps deterministically before asking the LLM for a confidence verdict

# Goal

The confidence pass is about a fifth of the calls of a run (8 of 28 on `delivery_router`, 72 of 320 on
linkding locally). Check mechanically what can be checked and keep the LLM verdict for a sample and for the
doubtful steps (ADR 0020 step 7).

# Findings

* Per-pass tables of `issues/behaviour_coverage_domains.md`: confidence is 72 calls and 17 minutes of 118
  locally on linkding; it was not run on the deepseek baseline.
* A step cites code locations and names identifiers and entities; grounding already drops references to files
  outside the feature (`use_cases/grounding.rs`).

# Approach

1. Checks without an LLM: the cited lines exist, the identifiers a step names appear at those lines, the
   entities it names are in its slice.
2. Score from the checks; send to the LLM only the steps that fail one, plus a sample (`--confidence-sample`).
3. Measure on saved runs how often the deterministic verdict and the LLM verdict agree.
4. Keep the score scale and the 50% flag so the report and the MCP server do not change.

# Resources

* `crates/retrodoc-pipeline/src/confidence/`, `use_cases/grounding.rs`, `report.rs`; ADRs `0011`, `0020`.

# Hints

* The fingerprint of the confidence model (`confidence_model`) must keep clearing the scores when the model
  changes; a purely deterministic score has no model.
