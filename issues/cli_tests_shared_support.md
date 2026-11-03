---
priority: low
depends_on: []
why: 'test hygiene like `openrouter_tests_shared_fixtures` (high review of `framework_fixtures_through_generate`, 2026-10-30)'
---
# Move the CLI tests onto the shared test support

# Goal
`crates/retrodoc-cli/tests/support` (a fake chat-completions server scripted per pass, `repo` and `generate`
helpers) was added for the fixtures of `framework_fixtures_through_generate`. The older CLI tests keep their own
copies, so a change to how a test repo is built or `generate` is run has to be made in several places.

# Findings (code read on 2026-10-30, high review of `framework_fixtures_through_generate`)
* `git()` is defined in `tests/support/mod.rs`, `tests/pass_models.rs`, `tests/source_hashes.rs` and
  `tests/stack_tables.rs`; `repo()` in `stack_tables.rs` and `source_hashes.rs`; `generate()` in `source_hashes.rs`.
* `pass_models.rs` and `source_hashes.rs` each start a one-shot fake server that reads a request with a single
  `read` of 64 KB: a larger or split request is cut, which the support server avoids (it reads by
  `Content-Length`).

# Approach
1. Make those tests `mod support;` and use its `git`, `repo` and `generate`, widening `repo` where a test needs
   another `retrodoc.toml` (a pass section, no server).
2. Replace the one-shot servers by `FakeLlm::start` with a fixed answer, keeping what each test counts.

# Resources
* `crates/retrodoc-cli/tests/support/mod.rs`, `docs/dev/conventions.md` (test patterns)

# Hints
* No behaviour change: the tests must assert the same things before and after.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
`pass_models.rs`, `source_hashes.rs` and `stack_tables.rs` drop their own `git`, `repo`, `generate` and one-shot
servers for `tests/support`; same assertions. Out of scope: the other CLI tests (they already use `support` or
need no repo). The support grows a `retrodoc.toml` of the test's choosing, extra CLI arguments and a server that
answers an HTTP error status (`source_hashes` needs a 400: a 500 is retried with backoff).

## Plan
1. [x] Shared support for the three tests: `repo_with_config` (files + any `retrodoc.toml`) under `repo`,
   `llm_section`, `run` (subcommand + arguments) under `retrodoc`, `FakeLlm::failing(status)`; the three tests use
   them and keep their counts and assertions; verified by the unchanged tests passing and `just check`. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 9f35cf6ab9cfd098c35336a4a6e953e67b4de094

## Deliverable 1 summary
done: `pass_models`, `source_hashes` and `stack_tables` run on `tests/support` (`repo_with_config`, `llm_section`,
`run`, `FakeLlm::failing`); their own `git`, `repo`, `generate` and 64 KB one-shot servers are gone.

| check | result |
|---|---|
| tests | pass (same assertions) |
| lint | pass |
| specs | pass |
| docs | `docs/dev/conventions.md` (test patterns) |
| code review | see below |

red: none (refactor under unchanged green tests)
Code review: no findings

not verified: none
decisions needed: none
