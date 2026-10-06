# Smoke test: false "conclusive" and a rerun that still calls the LLM

# Goal

Make the smoke test verdict reliable: it must no longer declare "conclusive" a run whose logs contain WARNs,
and it must check that the second `generate` makes no LLM call, which is now measurable.

# Findings

Token tracking smoke test (small Rust repo of 17 files, local `qwen3.6:35b-a3b`): verdict
`SMOKE TEST CONCLUSIVE: ... no warning ...`, although both logs contain WARN lines (chunk_check, actors,
unparseable JSON response, "LLM answered with no use case").

* Cause: `evaluate` (`.claude/skills/smoke-test/smoke.rs`) filters lines containing `" WARN "`. The `tracing`
  subscriber writes ANSI color codes even when stderr is redirected to a file: the level is followed by an
  escape code, not a space, so nothing matches.
* Criterion 3 ("second run = 0 files written") does not see that the second run made 2 LLM calls
  (`use-cases`, 922 input tokens). Since `d91bde4`, `.retrodoc/cache/usage.json` gives the number of calls per
  run.

# Approach

* Strip ANSI sequences before filtering (or disable the subscriber's colors when stderr is not a terminal,
  which also helps reading the logs). A test (`test_smoke.rs`) with a colored line must fail before the fix.
* Add a criterion: the second run made no LLM call (read the last entry of `usage.json`, or the `LLM usage:`
  line of the log). Decide whether it is blocking or only reported until `issues/smoke_empty_feature_retry.md`
  is handled.

# Resources

* `.claude/skills/smoke-test/smoke.rs`, `test_smoke.rs`, `SKILL.md` ("Conclusive means" criteria)
* `crates/retrodoc-pipeline/src/usage_log.rs` (`usage.json` format)
* `crates/retrodoc-cli/src/main.rs` (`tracing_subscriber` initialization)

# Hints

* Never name confidential test repos in committed files (numbers only).
* Harness tests are in Rust (`rust-script`), run by `just test-harness`.
