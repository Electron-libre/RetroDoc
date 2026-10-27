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
  `issues/done/hybrid_llama_server_as_local_llm.md` (deliverable 7).
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
* `issues/done/smoke_http_timeout_for_the_local_model.md`, `issues/done/hybrid_llama_server_as_local_llm.md`.
* `.claude/skills/smoke-test/smoke.rs` (`LLM_TIMEOUT_SECS`), `.claude/skills/quality-benchmark/benchmark.rs`
  (`LOCAL_TIMEOUT_SECS`).
* ADR 0002 (one provider, any OpenAI-compatible server).

# Hints
* A hosted provider that hangs must still fail in a reasonable time: don't raise the default for everyone.

# Tracking

Plan: validated

## Understanding
Without `llm.timeout_secs`, a loopback `base_url` gets 900 s, everything else keeps 120 s (`specs/llm-request-timeout.md`).
Candidate c is already done (655d1dc: a timeout is not retried); b is moot (`init` takes no endpoint). Means: a.
Sizing: longest local call measured ~160 s (use-cases, hybrid); the 8,192-token cap at iGPU speed (~15 tok/s) is ~550 s.

## Plan
1. [x] Local default timeout: `request_timeout` picks 900 s for a loopback host, 120 s otherwise; stale doc of
   `timeout_secs` (retry) fixed, `docs/crates/llm.md` and ADR 0012 updated; verified by unit tests on
   `request_timeout` and the existing no-retry test; makes true LTO-1 to LTO-4.
2. [x] Scripts rely on the default: smoke and benchmark stop writing `timeout_secs` unless `--timeout-secs` is
   given; verified by their `just test-harness` tests.
3. [x] Fixes of the `high` review: the timeout error gives the timeout that applied (LTO-4 amended), stale doc
   comments, a benchmark note for a `--base-url` on another host.

## Decisions
- Mode: autonomous
- Base commit: 46d7e407947ffc3c7356eb748853acda53c85140
- Q1: loopback hosts only get the long default (a server elsewhere on the network sets `llm.timeout_secs`).
- Q2: the scripts write `timeout_secs` only when `--timeout-secs` is given.

## Deliverable 1
- done: no `timeout_secs` + loopback `base_url` gives 900 s, else 120 s; spec LTO accepted; docs (config.rs, llm.md,
  ADR 0012, README) updated.
- checks: `just check` ok, `just test-harness` ok, specs traced (LTO-1 to LTO-4).
- red: `a_loopback_server_defaults_to_fifteen_minutes`: left 120s, right 900s.
- Code review (low): no findings.
- not verified: no real run against a local server with the new default.

## Deliverable 2
- done: smoke and benchmark write `timeout_secs` only with `--timeout-secs`; their docs and SKILL.md say so.
- checks: `just check` ok, `just test-harness` ok.
- red: smoke `written config leaves the timeout to the default of a local server` FAIL (`timeout_secs = 900` written);
  benchmark `leaves the timeout to the default of a local server` FAIL.
- Code review (low): no findings.
- not verified: no smoke or benchmark run against a local server with the new default.

## Deliverable 3 (fixes of the high review of 46d7e40..241f425)
- done: timeout error says `timed out after <timeout>`; `DEFAULT_REQUEST_TIMEOUT` and `patch_config` docs fixed;
  quality-benchmark SKILL.md notes `--timeout-secs` for a non-loopback `--base-url`.
- checks: `just check` ok, `just test-harness` ok.
- red: `a_timeout_is_named_and_not_retried` asserts `timed out after 200ms`, absent from the old message.
- High review findings not fixed:
  - non-loopback `--base-url` in the scripts gets 120 s: decision Q2; documented in both SKILL.md.
  - smoke.rs does not validate `--timeout-secs`: predates this issue.
  - `0.0.0.0`, `::ffff:127.0.0.1`, `*.localhost` not loopback: outside LTO-1 as validated (Q1).
  - `is_loopback_url` could match on `Url::host()`: needs a direct `url` dependency for a few lines.
  - positional timeout of the test-only `patch-config`: only the test calls it.
  - duplicated `[llm]` patching across smoke.rs and benchmark.rs: predates this issue.
- Code review (low): no findings.
- not verified: no real run against a local server with the new default.
