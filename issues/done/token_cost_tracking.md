# Token and cost tracking

# Goal

Know what a `generate` costs: number of LLM calls, input and output tokens, duration, per pass and per model,
then an optional amount. Today nothing is counted, and the per-pass timings of the 2026-10-02 run on the test
Rails repo were lost with its log. This is item 7 of phase 8 (`PLAN.md` §7.2), the only one still open.

# Approach

Read the `usage` field (`prompt_tokens`, `completion_tokens`) that OpenRouter, Gemini and Ollama return and
that the client ignores. Aggregate in a provider wrapper, like `HeartbeatProvider`, then print a recap at the
end of `generate`.

# Resources

* `PLAN.md` §7.2, item 7 (starting description and existing measurements)
* `crates/retrodoc-llm`: `CompletionResponse` (`content`, `model`), `OpenRouterProvider`, `HeartbeatProvider`
* `crates/retrodoc-pipeline/src/progress.rs` (one log line per unit) and `repo_map` (`estimate_repo_map`)
* `crates/retrodoc-core/src/config.rs` (`[llm]` section)
* ADR `0011` (cost control) and `0012` (LLM request resilience)

# Hints

* No hard-coded prices in the code: optional `llm.price_per_mtok_in` / `llm.price_per_mtok_out` in
  `retrodoc.toml`. Without them, no amount, only tokens.
* A server may not return `usage`: the field is optional, so we count the calls and say so (no invented
  tokens).
* The aggregation must hold with `llm.concurrency` > 1 (atomic counters or a lock, no assumed order).
* Retries count: a retried call consumes tokens twice.
* Tests use a fake `LlmProvider` (see `CountingProvider` in `repo_map/tests.rs`).
* Never name confidential test repos in committed files (numbers only).

# Tracking

## Plan (validated)

1. [x] **Capture**: `CompletionResponse` carries an optional `usage`; `OpenRouterProvider` reads it. Test on
   JSON responses with and without `usage`. Code in `crates/retrodoc-llm/src/usage.rs`; a missing,
   incomplete or malformed `usage` gives `None` without failing the response.
2. [x] **Per-pass aggregation**: shared `UsageTracker` (current pass via `set_pass`) + `UsageProvider<P>`
   that accumulates calls, in/out tokens, duration, calls without `usage`, per pass and per model. Test with
   a fake provider, including a concurrent case and a case without `usage`. Wired into the CLI in deliverable 3.
   Limit: failed calls and `OpenRouterProvider`'s internal retries are not counted, so the tokens are a lower
   bound with an unstable server (to document in deliverable 5).
3. [x] **Recap** at the end of `generate` and of each standalone command that calls the LLM (`roles`,
   `glossary`, `entry-points`, `actors`; `surface` does not call the LLM): table per pass + total. Details
   written to `.retrodoc/cache/usage.json` (last 20 runs, `command` field), not to `run-metadata.json`.
   A run with no call is not recorded; a failing command shows what it spent.
4. [~] **Optional prices**: abandoned (see Decisions). The recap only gives tokens.
5. [x] **Docs**: `PLAN.md` §7.2 item 7, `CLAUDE.md`, `docs/ARCHITECTURE.md`, ADR `0015`.

## Decisions

* The recap is **not** written to `_retrodoc/run-metadata.json` (it would change on every run and break
  rendering idempotence) but to `.retrodoc/cache/usage.json`.
* A recap for each command that calls the LLM, not only `generate`.
* `usage.json` history: last 20 runs.
* No prices or amounts: only the token cost is tracked (deliverable 4 abandoned, no
  `llm.price_per_mtok_in/out`). The amount is computed by hand from the tokens and the provider's rate.

## Smoke test

Small Rust repo (17 files, local `qwen3.6:35b-a3b`): recap and `usage.json` correct (37 calls, 22,881 input
tokens, 7,830 output tokens, 4m34s on the first run), idempotent rendering (0 files on the second run). The
script's "conclusive" verdict is a false positive (WARNs not detected because of ANSI codes) and the second
run made 2 LLM calls: tracked in `issues/smoke_test_false_positive.md` and
`issues/smoke_empty_feature_retry.md`.

## Out of scope (next issue)

Pre-run estimate for the whole pipeline: it requires real measurements (feature/use case ratios per file), so
to be done once this tracking is delivered and a few runs are recorded. See `PLAN.md` §7.2 item 7, step 2.
