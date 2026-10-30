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
