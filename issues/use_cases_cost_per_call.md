# Cut the time of the use cases pass

# Goal

The time of a run is in the use cases pass, not in the repo map: the new `surface` path removes calls but the
use cases still take most of the minutes. Find how to make the pass cheaper per use case without losing grounding.

# Findings

* linkding, local model: use cases 72.6 of 118.4 minutes (80 calls, 265k tokens in); deepseek 10.4 of 14.4
  minutes (`issues/behaviour_coverage_domains.md`).
* `delivery_router` local, `surface`: 8 use case calls took 7.5 of 12 minutes, with 22k tokens in and 15k out:
  the output dominates.
* One call per feature writes every use case of the feature with steps, actors, narrative and citations.

# Approach

1. Measure where the output tokens go (steps, narrative, citations) on saved runs.
2. Candidates, to measure one by one: shorter answer shape (narrative derived from steps), batching small
   features, concurrency for the pass (`llm.concurrency` applies to the repo map only), a smaller excerpt of
   code per entry point.
3. Check each against the benchmark (business-language rate, confidence, use cases found) and the calls and
   minutes per pass.

# Resources

* `crates/retrodoc-pipeline/src/use_cases/`, `slices.rs`, `usage_log.rs`; skills `quality-benchmark`,
  `smoke-test`.

# Hints

* Re-runs must stay no-ops: any new shape of the answer changes the fingerprints, so a `generate --force`
  is needed after the change.

# Tracking

1. [x] Use cases in parallel: `UseCaseContext.concurrency`; the requests of the features go out `concurrency`
   at a time, the answers are used in the order of the features (same result and fingerprints as one at a
   time); a failure keeps the features before it. Tests with a probe provider (same output at 1 and 3, calls
   in flight bounded, resume after a failure).
2. [x] Wiring and docs: `generate` passes `[llm.passes.use-cases] concurrency` (else `[llm] concurrency`);
   `config.rs`, `README.md`, `CLAUDE.md`, `docs/ARCHITECTURE.md`.
3. [ ] Measure: three `surface` runs of `delivery_router` on deepseek with concurrency 4, the use cases pass
   time against the saved runs (28-48 s); one linkding run only with the user's agreement (about $0.25).
4. [ ] Confidence in parallel, if the measure justifies it (same pattern); the features pass is too short.

## Decisions

* Lever A (concurrency) first; lever B (shape of the answer, batching, smaller excerpts) is not in this plan,
  to be reopened if the time stays too long (user, 2026-10-10).
* Paid runs sparingly: `delivery_router` on deepseek only, linkding after asking. Local runs need
  `OLLAMA_NUM_PARALLEL` on the server (the user's setting); measures are on the hosted model.
