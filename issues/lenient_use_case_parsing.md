# Use case responses rejected: "invalid type: map, expected a string"

# Goal

Stop dropping whole features because the LLM returns an object where `RawUseCase` expects a string, and make
the smoke test conclusive (criteria "no WARN" and "second `generate` = 0 files written").

# Findings

Smoke test of phases 7 and 8 on a small Rails repo (72 source files, local `qwen3.6:35b-a3b`,
`reasoning_effort = "none"`): 2 domains, 35 features, 109 use cases, 78% confidence. But:

* 16 WARNs on the first run, about ten of them `invalid type: map, expected a string` in the use cases pass
  (about 7 features out of 35 affected, 5 dropped after the 2 `complete_json` attempts); the rest: truncated
  JSON (`expected , or }`, `EOF while parsing a string`) and a network failure recovered on retry.
* A dropped feature has no use case (0% in the report). Another one (`base-presenter-inheritance`) gets
  "no use case" twice in a row.
* The second run is not a no-op: dropped units are retried (intended), some succeed and write new pages
  (14 files). This is not a rendering idempotence defect, it is the effect of the first run's failures.

The faulty field is not identified: the WARN only gives the line/column, and the raw response is only logged at
debug level. Candidates in `use_cases/grounding.rs`: `RawStep.action` / `description` (`String`),
`RawUseCase.description`, `primary_actor`, `entry_points: Vec<String>` (elements that are objects instead of
strings). The low columns (17-23) rather point to a step field.

# Approach

First read the raw response to learn which shape the model produces, then make deserialization lenient for
that field (accept a string or an object, and convert the object to text) instead of rejecting the whole
response. A malformed field must cost that field, not the feature.

# Resources

* `crates/retrodoc-pipeline/src/use_cases/grounding.rs` (`RawUseCases`, `RawStep`, `RawActor`, `RawSourceRef`)
* `crates/retrodoc-pipeline/src/response.rs` (`complete_json`, `parse_json_response`, "lenient parsing")
* `crates/retrodoc-pipeline/src/use_cases/prompt.rs` (schema requested from the LLM)
* ADR `0006` (lenient LLM output handling)
* Smoke test logs: `/tmp/retrodoc-smoke-smoke-src-rails.run1.log` (may have been purged), clone with its
  caches in `/tmp/retrodoc-smoke-smoke-src-rails`

# Hints

* Reproduce before fixing: rerun `generate` on the smoke clone at debug level
  (`RUST_LOG=retrodoc_pipeline::response=debug`) after removing the use cases cache of a feature that
  failed; the `unparseable LLM response (raw)` log contains the response. Test repos are confidential: do not
  copy a response or code into the issue, a test or a commit, write a synthetic example.
* Tests use a fake `LlmProvider` (see `SequenceProvider` in `response.rs` and `CountingProvider` in
  `repo_map/tests.rs`).
* Truncated JSON (`finish_reason=length`, 8,192 output token limit) is a different problem: only handle it
  here if the raw response shows it is related.
* The leniency must stay bounded: an unexpected object is converted to text, not silently ignored; a
  `debug!` reports the corrected field.
* Do not change the prompt before measuring: the goal is to tell a prompt error from a model limit
  (see `PLAN.md` §7.1, caveats).

# Tracking

## Plan (to be validated)

1. [x] **Diagnosis** (the field is `primary_actor`, returned as an object `{name, kind}`, the shape of a step's `actor`; the prompt only says "the known human actor", so the model copies the neighbouring shape; systematic on the failing features, same error positions in all WARNs): get the raw response of a failing feature, identify the field and the shape (object with
   which keys), and deduce whether it is an isolated or a systematic case. Record the shape (without
   confidential content) in this issue.
2. [x] **Behavior test**: fake provider that returns a response with this field as an object; the feature
   must get its use cases, without a retry.
3. [x] **Lenient deserialization** for this field (string or object), with `debug!`; tests for the string,
   object, and unusable object cases.
4. [ ] **Second smoke test** on the same subset: measure the number of WARNs and whether the second run writes
   0 files; record the numbers here and in `PLAN.md` §7.
5. [ ] **Docs**: `PLAN.md` §7 (smoke test result), ADR `0006` if the leniency rule changes, doc of the
   `use_cases` module.

## Out of scope

* JSON truncated by the output token limit (other cause, other possible fix: split the feature or raise the
  limit).
* Passing `--max-files` to the smoke script (`smoke.rs run` only accepts `--model` and `--base-url`), to be
  opened separately if the file budget should be covered.
