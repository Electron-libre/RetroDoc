---
priority: medium
why: 'a product principle with no evidence behind it; free in money but hours of machine time, so after the hosted baseline'
depends_on: [benchmark_baseline_at_head]
on_hold: 'the local model is not a priority for now (user, 2026-11-04)'
---
# Run a complete benchmark on the local model

# Goal
`PRODUCT.md` holds that a small local model is a valid target, at a lower quality and a longer run. No complete
benchmark run of an application on the local model has finished: the only attempt on linkding failed. Expected: one
complete local `generate` of linkding scored by the benchmark, with its duration and figures next to the hosted
baseline, then either the principle kept with those figures or amended.

# Findings
* 2026-10-27, linkding, `qwen3.6:35b-a3b` on the hybrid llama-server (`just benchmark --local hybrid --runs 1`): both
  series failed in the use cases pass after 66 min on the 120 s timeout; use case calls averaged 85 s, some over
  120 s.
* Fixed since by `issues/done/default_timeout_for_a_local_server.md`: a loopback `base_url` gets 900 s; no local
  benchmark was run after.
* The hybrid server is about twice as fast as the iGPU Ollama instance (2026-10-26, `delivery_router`).
* Earlier local runs were smoke tests (`delivery_router`, linkding, a large Rails application), not scored.

# Approach
1. With the user's go (about 4 hours of machine time for a linkding series), one `just benchmark --local hybrid
   --runs 1` series, hidden only, at the commit of `benchmark_baseline_at_head`.
2. If it fails, record where and why, and fix only what blocks completion (timeouts, answer caps), each fix with its
   test.
3. Compare with the hosted baseline: recall, precision, business language, duration per pass; write it in the ledger,
   `docs/findings.md` and the constraints of `PRODUCT.md`.
4. If the quality is far below the hosted run, propose to the user the wording of the principle (a local model for
   privacy at a stated quality) or a per-pass split (local for reading, hosted for writing).

# Milestone
A local model, measured

# Resources
* Skills `quality-benchmark`, `smoke-test`; the machine setup notes of the user (local servers and ports).
* `specs/llm-request-timeout.md`, `docs/crates/llm.md`.

# Hints
* Ask before a run of several hours; run it in the background and check the log, do not poll.
* A local run judged by the local model is not comparable with a hosted judge: judge the local run's output with the
  hosted judge (`retrodoc benchmark --judge` on the saved run, cents).
