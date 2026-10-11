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

# Tracking

1. [x] Deterministic checks of one step (`confidence/checks.rs`, pure): `Pass` / `Doubtful` / `Broken`; cited lines exist, named identifiers (heuristic extraction from `action` + `description`) appear at the cited lines, named entities appear in the slice; entity names passed in from the saved glossary.
2. [ ] Wire the checks into `score_confidence`: `Pass` is supported and `Broken` unsupported without a call, only `Doubtful` steps go to the LLM, `--confidence-sample` gives a second full LLM opinion on fully passing use cases, mechanical scores recomputed every run, `confidence_model` clears LLM scores only.
3. [ ] Agreement measure (`confidence_agreement` example, no LLM): deterministic vs saved LLM score per use case on saved runs (use-case level, since per-step verdicts are not saved).
4. [ ] Docs (`update-docs`) and a short ADR extending 0020.

## Decisions

* Identifiers a step names: extracted by heuristic from its text (backticks, `snake_case`, `CamelCase`, `foo()`); no new structured field (it would change the model and invalidate caches).
* The deterministic check filters: a fully passing step is `supported` without an LLM; unreadable cited file or lines out of range is `unsupported` without an LLM (a fact); everything else goes to the LLM.
* `--confidence-sample N` keeps its meaning and applies to fully passing use cases, which get a second LLM opinion.
* An ungrounded step (no reference) is capped at `UNGROUNDED_STEP_CAP` without a call (`StepCheck::Ungrounded`).
* A mechanical score is recomputed on every run (free, code may have changed), so it needs no fingerprint.
