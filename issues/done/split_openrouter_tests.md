# Move the OpenRouter provider tests into files of their own

# Goal
`crates/retrodoc-llm/src/openrouter.rs` holds the provider and about 740 lines of its tests in one file, long
to read and to review. Move the tests apart, one file per theme, with no behaviour change: `just check` stays
green.

# Findings (exploration of 2026-10-29, at 2a9c27a)
* Cut from `issues/done/split_largest_files.md` (one issue per crate, to stay under the size limit).
* `crates/retrodoc-llm/src/openrouter.rs`, 1,281 lines: production code at 1-540; tests at 541-1281 with
  themes already apart: config and endpoints (546-650), retry delays (651-698), completion (699-986), schema and
  temperature fallbacks with their fake server (987-1178), timeouts `LTO-1` to `LTO-4` (1179-1281).
* The production code has no seam worth a split (decision of the planning of `split_largest_files`, 2026-10-29).

# Approach
1. `openrouter.rs` becomes `openrouter/mod.rs` (a `git mv`) plus `openrouter/tests/`: a `mod.rs` with the shared
   helpers and one file per theme (config, retry, completion, fallbacks, timeouts).
2. About 1,480 changed lines, the tests being dedented: two deliverables, the helpers with config and retry
   (about 500), then completion, fallbacks and timeouts (about 980).
3. One commit per deliverable.

# Resources
* `issues/done/split_largest_files.md` (the issue it was cut from, and its plan)
* `issues/done/refactor_code_hygiene.md` (how the earlier splits were made)
* `docs/crates/llm.md` (module list to update)
* `specs/` (the `// spec: LTO-n` lines move with their tests; `just check-specs` must still find them)

# Hints
* after split_render_markdown
* Keep the public API of the crate (re-export from `mod.rs`).
* The line numbers above move with every commit; read the file again at planning.

# Tracking

Plan: validated

## Understanding
Split the tests of `crates/retrodoc-llm/src/openrouter.rs` out of the file: `openrouter/mod.rs` keeps the provider,
`openrouter/tests/` holds the shared helpers and one file per theme; pure move, public API unchanged. Out of scope:
any change of the production code or of a test body.

## Plan
1. [x] Helpers, config and retry tests apart: `git mv openrouter.rs openrouter/mod.rs`; `request()` in `openrouter/tests/mod.rs` (`pub(super)`), config and endpoints tests in `tests/config.rs`, retry delay tests in `tests/retry.rs`; the completion, fallback and timeout tests stay inline in `mod.rs` for now, renamed `mod rest_tests` (body untouched) and importing `request`; `docs/crates/llm.md` points to `openrouter/`; verified by `just check` and the moved lines identical modulo whitespace (~500 lines).
2. [x] Completion, fallbacks and timeouts apart: `rest_tests` → `tests/completion.rs`, `tests/fallbacks.rs` (with `serve`, `http` and the schema and temperature helpers), `tests/timeouts.rs` (the `// spec: LTO-n` lines move with their tests), dedented; verified by `just check` (specs found) and the test count unchanged (~980 lines).

## Decisions
- Mode: autonomous
- Base commit: fc03730

## Deliverable 1: helpers, config and retry tests apart
- done: `openrouter.rs` → `openrouter/mod.rs` (git mv); `request()` in `openrouter/tests/mod.rs` (`pub(super)`), config and endpoint tests in `tests/config.rs`, retry delay tests in `tests/retry.rs`, imports named; the other tests stay inline as `mod rest_tests` (body untouched); `docs/crates/llm.md` points to `openrouter/`.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved lines identical (sorted line comparison) | yes, apart from imports, `mod` lines and `pub(super)` |
  | test count of the crate | 46, unchanged |
  | docs (`docs/crates/llm.md`) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none

## Deliverable 2: completion, fallbacks and timeouts apart
- done: `rest_tests` → `openrouter/tests/completion.rs`, `tests/fallbacks.rs` (with `serve`, `http` and the schema and temperature helpers), `tests/timeouts.rs` (with `LTO-1` to `LTO-4`), dedented, imports named; `openrouter/mod.rs` keeps only the provider (542 lines).
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass, `LTO-1` to `LTO-4` found |
  | moved lines identical (sorted line comparison) | yes, apart from imports, `mod` lines, `pub(super)` and two `let payload` lines rewrapped by rustfmt |
  | test count of the crate | 46, unchanged |
  | docs (`docs/crates/llm.md`) | already right (deliverable 1) |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: none
- decisions needed: none

## High review (fc03730..db3d41b)
- 6 findings, no correctness bug: the split is a pure move (production code and the tests identical, 46 tests, `LTO-1` to `LTO-4` traced).
  - pre-existing, outside a no-behaviour-change issue: the hand-built mock server repeated in `completion.rs` and `timeouts.rs` while `serve`/`http` sit in `fallbacks.rs`; the `OpenRouterProvider` literal repeated about 10 times; `CompletionRequest` literals that could use `..request()`; `std::thread::sleep` in the async `LTO-4` test. Left to the user (issue or not).
  - the end-to-end 429 test stays in `completion.rs`: it goes through `complete`, `retry.rs` holds the delay functions, as the issue's themes said.
  - follow-up: `issues/openrouter_tests_shared_fixtures.md` for the pre-existing findings; issue closed on the user's agreement.
