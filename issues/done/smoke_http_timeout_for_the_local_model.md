# Set an HTTP timeout in the smoke test that the local model can meet

# Goal
The smoke test must not fail on a large repository because of the default HTTP timeout. The default
(`llm.timeout_secs`, two minutes) cuts a long answer of the local model and the log says "network failure",
which looks like a lost server. Make the smoke test write a timeout that fits the local model, and make the
cut readable in the log.

# Findings (smoke run of 2026-10-15, a large Rails application, local `qwen3.6:35b-a3b`)
* The entry points call on a large routes file (about 3,400 tokens in, about 4,300 out) takes longer than
  two minutes at about 38 tokens per second. The client cut it, retried three times (500ms, 1s, 2s) and the
  `generate` failed with "failed to build the entry points inventory" after 51 minutes of work, twice.
* The log line was `OpenRouter network failure … error sending request`: nothing says it is a timeout, and the
  server was up and still generating.
* `llm.timeout_secs = 900` in the clone's `retrodoc.toml` let the run finish; `smoke.rs` does not write it.

# Approach
1. `smoke.rs` writes `timeout_secs` in the `[llm]` section it generates (a few minutes' margin over the
   longest answer of the local model), with a test of the written config.
2. The retry warning names a timeout when the error is one (`reqwest::Error::is_timeout`), with the delay.
3. Check whether a retry after a timeout is useful: the same prompt will take as long again.

# Resources
* `.claude/skills/smoke-test/smoke.rs`, `SKILL.md` (local LLM defaults)
* `crates/retrodoc-llm/src/openrouter.rs` (`request_timeout`, retry)

# Hints
* The repository under test is confidential: no name or path of it in a committed file.

# Tracking
1. [x] `smoke.rs` writes `timeout_secs = 900` in `[llm]` (`--timeout-secs` overrides it), keeping the per-pass tables; tested in `test_smoke.rs`.
2. [x] A timeout while waiting for the answer says so (and names `llm.timeout_secs`).
3. [x] Such a timeout is not retried (the same prompt takes as long again); a connect timeout still is.

## Decisions
* Not verified by a real smoke run on a large application.
