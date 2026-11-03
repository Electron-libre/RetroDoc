---
priority: low
depends_on: []
why: 'test hygiene with no user-visible effect (high review of `split_openrouter_tests`, 2026-10-29)'
---
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
* `issues/done/split_openrouter_tests.md` (the split and its high review)

# Hints
* The tests that check the exact bytes of a request (headers, `max_tokens`, `reasoning_effort`) need the requests
  received: keep that channel in the shared `serve`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
Share the mock server (`serve`, `http`), the provider (`provider(endpoint)`) and the requests in `tests/mod.rs`, and
make `LTO-4` read its accept count without a sleep. Test code only: no production change, same test count. No spec
(pure refactor of tests).

## Plan
1. [x] Shared fixtures and a deterministic `LTO-4`: `serve`/`http`/`OK_BODY`/`provider` move to `tests/mod.rs`, the
   hand-written servers and literals of `completion.rs`, `fallbacks.rs`, `timeouts.rs` use them; `LTO-4` opens a
   sentinel connection after the error and counts the connections the server accepted before it (the backlog is
   FIFO), with no sleep; verified by `just check` green and the same test count (~250 lines). (autonomous)

2. [x] High review fixes: `serve` reads the whole request (headers then `Content-Length` body), one test sends a user message again and checks its mapping, `LTO-4` waits with `recv_timeout`, `OK_BODY` replaces a local copy. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 39f550cf560a45c78d52c7a87849cb9a0f387ed4
- The 429 response with `Retry-After` is passed raw to `serve` (`http()` takes no headers).

## Deliverable 1 summary
done: `serve`, `http`, `OK_BODY`, `provider` shared in `tests/mod.rs`; hand-written servers and provider literals
gone; `LTO-4` counts the connections accepted before a sentinel, no sleep.

| check | result |
|---|---|
| tests | 46 before, 46 after |
| `just check` | green |
| docs | no doc describes these fixtures |
| code review | see below |

red: none (refactor of tests, no new behaviour)

Code review: no findings (`low` reads no test file: the diff is test code only)

not verified: that `LTO-4` would fail on a retry (no production change made to prove it)
decisions needed: none

## High review (39f550c..a1cd352): 7 findings
1. Empty `messages` no longer exercised the message mapping: fixed (the `max_tokens` test sends "hi" and checks it).
2. Single `read` could lose a body sent in its own segment: fixed (`read_request`).
3. `LTO-4` waited with no deadline: fixed (`recv_timeout` of 5 s; the connect to loopback is immediate).
4. `with_endpoint` could go: not fixed, the issue forbids a change of the production code.
5. `OK_PAYLOAD` duplicated `OK_BODY`: fixed.
6. Raw 429 response: kept, decided at validation.
7. A client built then dropped in `LTO-4`: not fixed, negligible in a test.

## Deliverable 2 summary
done: the fixes above. tests: 46, `just check` green. red: none (test hygiene).
Code review: no findings
