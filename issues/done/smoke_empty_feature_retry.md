# A feature with no use case is retried on every run

# Goal

Make a rerun on an unchanged repo not call the LLM again for a feature it already answered "no use case"
for, or make that retry an explicit, documented choice.

# Findings

Token tracking smoke test (small Rust repo of 17 files): one feature (out of 6) gets "LLM answered with no
use case" on both attempts, on every `generate`. It has no use case (0% confidence in the report) and is not
remembered as "processed": the second run makes 2 calls again (922 input tokens, 20 output tokens) while
everything else is reused. The token recap makes it visible (`LLM usage` of the second run).

Retrying a failed unit is intended (ADR 0005: resume), but here the LLM answered cleanly, it is not an
outage. The result is the same on every run as long as the model and the input do not change.

# Approach

To be decided first, before coding:
* either remember "no use case" in the pass cache (with the input fingerprint), so a rerun skips it, and
  `--force` or an input change reruns it;
* or keep the retry (the model may answer differently) and write it down in ADR 0005 and `PLAN.md`.

The first option has a cost: an empty answer caused by a bad model draw would be frozen until `--force`.
See also `issues/lenient_use_case_parsing.md`, which deals with responses rejected by the parser (different
cause, same symptom in the report).

# Resources

* `crates/retrodoc-pipeline/src/use_cases/` (per-feature cache, `LLM answered with no use case`)
* `crates/retrodoc-pipeline/src/fingerprints.rs`, `cache.rs`
* ADR `0005` (incremental re-run, fingerprints and resume)
* `.retrodoc/cache/usage.json` to measure the calls of a rerun

# Hints

* Reproduce with a fake `LlmProvider` that answers an empty list of use cases (see `CountingProvider` in
  `repo_map/tests.rs`): if option 1 is chosen, the second `build_use_cases` call must not call the provider.
* Never name confidential test repos in committed files (numbers only).

# Tracking

1. [x] Remember "no use case" in the use cases pass cache (fingerprint without saved use case); a rerun
   skips it, `--force` or an input change reruns it. Tests in `use_cases/tests.rs`; ADR 0005 updated.

## Decisions

* Option 1, restricted: only a reliable empty answer (both attempts parsed and empty) is remembered;
  unparseable answers and LLM errors are still retried.
* A feature whose use cases are all dropped by grounding is remembered too (same input, same result).
* The report is unchanged (the feature stays at 0% confidence).
* No new ADR: a paragraph in the Consequences of ADR 0005.
