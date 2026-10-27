# Give generate a default HTTP timeout that a local server can meet

# Goal
`retrodoc init` writes no `llm.timeout_secs`, so `generate` uses the default of 120 s. A local server answering
long structured outputs runs past it: the call is cut, retried with the same prompt, and the pass fails. Only
the smoke test and the benchmark scripts write 900 s, so a user running `init` then `generate` against a local
`base_url` hits the failure the scripts hide. Make the default fit a local server, without slowing the failure
of a hosted provider that is down.

# Findings (benchmark of 2026-10-27, linkding, `qwen3.6:35b-a3b-hybride` on the hybrid llama-server)
* Both series failed in the use-cases pass after 66 min: `the request timed out before the model finished its
  answer (raise llm.timeout_secs)`. The use case calls averaged 85 s (13 calls, 129,642 tokens in, 34,794 out,
  18m26s), some over 120 s.
* The Ollama iGPU instance generates about half as fast, so its calls run longer still.
* The same cut on a large Rails application led to `issues/done/smoke_http_timeout_for_the_local_model.md`,
  which fixed the smoke test only; `benchmark.rs` got the same 900 s in
  `issues/hybrid_llama_server_as_local_llm.md` (deliverable 7).
* The retry loop restarts the whole generation on a timeout (doc of `timeout_secs` in `config.rs`), so each
  failing call costs about three times the timeout before the pass fails.

# Approach
1. Measure the longest call per pass on a local server from saved `usage.json` files and run logs, to size the
   value instead of guessing.
2. Candidates (can be combined):
   a. a longer default when the endpoint is local (loopback `base_url`, no API key);
   b. `init` writes `timeout_secs` when it is pointed at a local server;
   c. no retry after a timeout, since the same prompt takes as long again.
3. Once the default fits, the scripts can stop writing their own value (or keep it as an override).

# Resources
* `crates/retrodoc-core/src/config.rs` (`timeout_secs`), `crates/retrodoc-llm/src/openrouter.rs` (timeout, retry).
* `issues/done/smoke_http_timeout_for_the_local_model.md`, `issues/hybrid_llama_server_as_local_llm.md`.
* `.claude/skills/smoke-test/smoke.rs` (`LLM_TIMEOUT_SECS`), `.claude/skills/quality-benchmark/benchmark.rs`
  (`LOCAL_TIMEOUT_SECS`).
* ADR 0002 (one provider, any OpenAI-compatible server).

# Hints
* A hosted provider that hangs must still fail in a reasonable time: don't raise the default for everyone.
