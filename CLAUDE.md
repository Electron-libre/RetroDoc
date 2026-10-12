# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

RetroDoc is a Rust CLI that uses LLM agents to catch up on a software project's documentation debt.
v1 (MVP) scope: **functional documentation only**, from a **local Git repo** (code + commit history +
existing Markdown docs), using **OpenRouter** or the **DeepSeek** API as LLM provider (same wire format). See `PRODUCT.md` for the long-term
product vision and `PLAN.md` for the v1 architecture, pipeline stages, and roadmap — read `PLAN.md` before
starting new work, it's the source of truth for what each roadmap phase covers and what's still out of
scope for v1 (technical/C4 docs, GitHub/GitLab/Jira connectors, multi-provider LLM).

Commit messages follow the Conventional Commits format described in `AGENTS.md`. Check `git log` to see
which roadmap phases from `PLAN.md` §5 are already done before assuming what's implemented.

## Commands

```sh
cargo build --workspace                 # build everything
cargo test --workspace                  # run all tests
cargo test -p retrodoc-pipeline         # run one crate's tests
cargo test -p retrodoc-pipeline repo_map::tests::builds_bottom_up  # run a single test
cargo clippy --workspace --all-targets  # lint (see Lints below — must be warning-free)
cargo fmt --all                         # format
just check                              # fmt + clippy -D warnings + tests in one go
just test-harness                       # test the agent hooks and skills (needs rust-script)
just probe-schema <ollama|deepseek|gemini|openrouter>   # one HTTP call: does the server reject, ignore or obey a strict JSON schema
cargo run -q -p retrodoc-pipeline --example entry_points_replay -- benchmark/<repo>/entry-points.yaml <checkout> <runs dir>   # saved runs vs a hand-made entry points/actors reference (no LLM)
cargo run -q -p retrodoc-pipeline --example behaviour_baseline -- <run dir>...   # share of source files a behaviour reaches and calls/minutes per pass of saved runs (no LLM)
cargo run -q -p retrodoc-pipeline --example context_map_measure -- [--names] <run dir>...   # can bounded contexts be built from the file names: contexts, coverage, overlap of saved runs (no LLM)
```

Scripts and dev commands are `just` recipes (`justfile`) and `rust-script` files, not bash.

Registry note: this environment has no direct network access to `crates.io` (plain HTTP is blocked), but
`cargo build`/`cargo test`/`cargo clippy` work fine without `--offline` — cargo reaches the registry through
a path that works even though `curl https://crates.io` doesn't. Don't pass `--offline`; it fails on any
dependency not already in `~/.cargo/registry/cache`.

Manual CLI smoke test (no real OpenRouter call needed to check the error path):
```sh
cargo run -p retrodoc-cli -- init --path <target-repo>
cargo run -p retrodoc-cli -- scan --path <target-repo>
OPENROUTER_API_KEY=... cargo run -p retrodoc-cli -- generate --path <target-repo>
```

API keys may live in a git-ignored `.env` (read by the CLI at startup, `dotenvy`; variables already set win).
With `provider = "deepseek"` in `retrodoc.toml` the key is `DEEPSEEK_API_KEY`.

## Agent harness

Hooks in `.claude/settings.json` (`rust-script` files in `.claude/hooks/`, tested by `test_hooks.rs`):
- `format_rust.rs` (PostToolUse on Edit/Write): runs `cargo fmt --all` after a `.rs` file is edited.
- `clippy_gate.rs` (Stop): if `.rs` files changed, runs `cargo clippy --workspace --all-targets -- -D warnings`;
  on failure the output goes back to the agent, which must fix the code before finishing.

Skills in `.claude/skills/` (structure checked by `.claude/skills/test_skills.rs`):
- `issue-workflow`: how to work an `issues/*.md` file (reformulate, plan, then per deliverable test, code,
  docs, review, human validation, tracking, commit). Architecture decisions go to `docs/adr/`.
- `create-issue`: write a new `issues/*.md` with the template (Goal, Findings, Approach, Resources, Hints), validated by
  `check_issue.rs` (`just check-issue issues/<name>.md`).
- `prioritize-issues`: re-rank `issues/queue.md` (the ordered list of open issues; `just next-issue` prints the first)
  from the roadmap, ADRs, history and each issue, with a validation stop.
- `commit-message`: procedure to write a commit message per `AGENTS.md`, validated by `commit_check.rs`
  (`just check-commit` checks `HEAD`).
- `smoke-test`: end-to-end run on a real repo with the local Ollama and a verdict (`just smoke <repo>`).
- `quality-benchmark`: quality of the generated docs against a hand-written reference on a public repo, user docs hidden then shown, several runs, table with the change since the previous benchmark (`just benchmark benchmark/<repo>/reference.yaml`).
- `update-docs`: checklist to keep the documentation in sync after a change.

## Architecture

Cargo workspace, seven crates under `crates/`, dependency direction flows one way (no cycles):

```
retrodoc-cli ──> retrodoc-pipeline ──> retrodoc-ingest ──> retrodoc-core
             ──> retrodoc-llm      ──────────────────────> retrodoc-core
             ──> retrodoc-ingest
retrodoc-cli ──> retrodoc-render ──> retrodoc-core
retrodoc-cli ──> retrodoc-mcp ──> retrodoc-pipeline, retrodoc-ingest, retrodoc-core
```

Per-crate detail (modules, passes, behaviours that span passes) lives in `docs/crates/`; read the file
of the crate you are about to touch. High-level view: `docs/ARCHITECTURE.md`. Decisions: `docs/adr/`.

- **retrodoc-core**: domain model (`model.rs`) and `retrodoc.toml` config (`config.rs`). → `docs/crates/core-ingest.md`
- **retrodoc-ingest**: file walker, git history (one revwalk), existing Markdown docs, signals of the product
  brief, tree-sitter `structure.rs`. → `docs/crates/core-ingest.md`
- **retrodoc-llm**: `LlmProvider` trait, `OpenRouterProvider` (also serves DeepSeek), retry, structured output,
  token usage. → `docs/crates/llm.md`
- **retrodoc-pipeline**: the multi-pass generation pipeline, one module per pass (the `//!` doc of each module is
  the reference), plus incremental re-run, model per pass, resume, cost control, token accounting.
  → `docs/crates/pipeline.md`
- **retrodoc-render**: writes the Markdown/Mermaid docs, deterministic and idempotent. → `docs/crates/render-mcp-cli.md`
- **retrodoc-mcp**: read-only, LLM-free MCP server over the generated docs. → `docs/crates/render-mcp-cli.md`
- **retrodoc-cli**: `clap` subcommands (`init`, `scan`, `generate`, `render`, `report`, `roles`, `brief`, `glossary`,
  `entry-points`, `business-read`, `business-files`, `actors`, `surface`, `benchmark`, `benchmark-table`, `search`,
  `mcp`); logs go to stdout (stderr for `mcp`). → `docs/crates/render-mcp-cli.md`

### Conventions specific to this codebase

- Repositories used for smoke tests may be confidential: never name them, nor quote their paths or code, in
  committed files (docs, `PLAN.md`, `issues/`, commit messages). Describe them generically.
- Everything is in English: doc comments, user-facing CLI/error strings, commit messages, and `PLAN.md`/
  `PRODUCT.md`. Don't reintroduce French — an earlier revision of this repo was French throughout and its
  history was rewritten to English (see `backup-fr-history` if you ever need the old wording for reference).
- `clippy::pedantic` is enabled workspace-wide (`[workspace.lints.clippy]` in the root `Cargo.toml`; each
  crate opts in via `[lints] workspace = true`). Any new `pub fn` returning `Result` needs a `# Errors` doc
  section; getters with no side effects need `#[must_use]`. Keep clippy warning-free before committing.
- Pipeline code that calls an `LlmProvider` is tested with a fake in-`#[cfg(test)]` implementation of the
  trait (`FakeLlm` in `testing.rs`: canned or scripted answers, call count, recorded prompts) rather than hitting the network — follow that pattern for
  new pipeline passes instead of adding integration tests that need `OPENROUTER_API_KEY`.
- Filesystem-touching tests use `tempfile::tempdir()` and, where git history matters, actually run `git
  init`/`git commit` in the tempdir (see `git_history.rs` and `walker.rs` tests) rather than mocking `git2`.
