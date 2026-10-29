# Share the fixtures of the OpenRouter provider tests

# Goal
The tests of the OpenRouter provider, split by theme in `crates/retrodoc-llm/src/openrouter/tests/`, still copy
their mock server, their provider and their requests by hand, and one of them depends on timing. Share the
fixtures and make the timeout test deterministic, with no change of the production code: `just check` stays green
and the test count is unchanged.

# Findings (high review of `split_openrouter_tests`, 2026-10-29, at db3d41b)
* The one-shot mock HTTP server (`TcpListener::bind`, accept, read, write a raw response) is written by hand 5
  times in `tests/completion.rs` and once in `tests/timeouts.rs`, while `serve` and `http` already do it, private
  to `tests/fallbacks.rs`.
* The `OpenRouterProvider { ... }` literal with its 11 fields is repeated 10 times (`completion.rs` 5,
  `fallbacks.rs` 4, `timeouts.rs` 1), with 1 to 3 fields differing; `schema_provider` and `temperature_provider`
  in `fallbacks.rs` wrap the same literal.
* `CompletionRequest { ... }` literals remain in `completion.rs` (4) and `fallbacks.rs` (2) where the shared
  `request()` of `tests/mod.rs` could serve, with `..request()` when a field differs.
* `a_timeout_is_named_and_not_retried` (`LTO-4`, `tests/timeouts.rs`) is a `#[tokio::test]` that calls
  `std::thread::sleep(100 ms)` before reading the accept counter: the "not retried" check depends on timing and the
  sleep blocks the runtime thread.

# Approach
1. Move `serve` and `http` to `tests/mod.rs` (`pub(super)`) and use them in `completion.rs` and `timeouts.rs`
   where the test only needs canned responses and the requests received.
2. A shared `provider(endpoint)` helper in `tests/mod.rs`, overridden with struct-update syntax; `schema_provider`
   and `temperature_provider` built on it.
3. Replace the `CompletionRequest` literals by `request()` or `..request()`.
4. Make the `LTO-4` check wait on the server thread (join, or a channel) or on `tokio::time::sleep`, so the count
   is read once the server has seen every connection it will get.

# Resources
* `crates/retrodoc-llm/src/openrouter/tests/` (`mod.rs`, `completion.rs`, `fallbacks.rs`, `timeouts.rs`)
* `specs/` (the `// spec: LTO-n` lines stay with their tests)
* `docs/dev/conventions.md` (test patterns)
* `issues/split_openrouter_tests.md` (the split and its high review)

# Hints
* The tests that check the exact bytes of a request (headers, `max_tokens`, `reasoning_effort`) need the requests
  received: keep that channel in the shared `serve`.
