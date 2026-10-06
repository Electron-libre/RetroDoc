# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

RetroDoc is a Rust CLI that uses LLM agents to catch up on a software project's documentation debt.
v1 (MVP) scope: **functional documentation only**, from a **local Git repo** (code + commit history +
existing Markdown docs), using **OpenRouter** as the only LLM provider. See `PRODUCT.md` for the long-term
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

## Agent harness

Hooks in `.claude/settings.json` (`rust-script` files in `.claude/hooks/`, tested by `test_hooks.rs`):
- `format_rust.rs` (PostToolUse on Edit/Write): runs `cargo fmt --all` after a `.rs` file is edited.
- `clippy_gate.rs` (Stop): if `.rs` files changed, runs `cargo clippy --workspace --all-targets -- -D warnings`;
  on failure the output goes back to the agent, which must fix the code before finishing.

Skills in `.claude/skills/` (structure checked by `.claude/skills/test_skills.rs`):
- `issue-workflow`: how to work an `issues/*.md` file (reformulate, plan, then per deliverable test, code,
  docs, review, human validation, tracking, commit). Architecture decisions go to `docs/adr/`.
- `commit-message`: procedure to write a commit message per `AGENTS.md`, validated by `commit_check.rs`
  (`just check-commit` checks `HEAD`).
- `smoke-test`: end-to-end run on a real repo with the local Ollama and a verdict (`just smoke <repo>`).
- `update-docs`: checklist to keep the documentation in sync after a change.

## Architecture

Cargo workspace, six crates under `crates/`, dependency direction flows one way (no cycles):

```
retrodoc-cli ──> retrodoc-pipeline ──> retrodoc-ingest ──> retrodoc-core
             ──> retrodoc-llm      ──────────────────────> retrodoc-core
             ──> retrodoc-ingest
retrodoc-cli ──> retrodoc-render ──> retrodoc-core
```

- **retrodoc-core**: shared domain model (`Domain`/`Feature`/`UseCase`/`Step`/`ConfidenceScore` in
  `model.rs` — populated starting the "domains" phase, still empty in v1 so far) and `retrodoc.toml`
  config (`config.rs`).
- **retrodoc-ingest**: reads the target repo — file walker respecting `.gitignore` (`walker.rs`; classifies files as `Source`/`Test`/`Markdown`/`Other` —
  tests are detected by directory or file-name convention and kept out of the pipeline, which only
  consumes `Source`), one-pass
  git history per file via `git2` revwalk+diff (`git_history.rs`, not a `git log` per file — matters for
  perf on large repos), existing Markdown docs (`existing_docs.rs`). `run()` combines all three into an
  `IngestResult`.
- **retrodoc-llm**: `LlmProvider` trait abstraction (kept provider-agnostic even though only OpenRouter is
  implemented in v1) + `OpenRouterProvider`, a real `reqwest` HTTP client with exponential-backoff retry on
  429/5xx that honors the delay the server asks for (`Retry-After`, Google's `retryDelay`; up to 120 s, a longer
  one is an error). Per-request HTTP timeout is 120 s unless `llm.timeout_secs` is set (a slow local
  model writing a long JSON answer needs more; a timeout restarts the whole generation on retry). `CompletionResponse.usage`
  is the optional token count the server reports (`usage.rs`: a missing or malformed block is `None`, never estimated);
  `UsageProvider` + `UsageTracker` count answered calls and tokens per pass and per model (concurrency-safe; the CLI names
  the pass with `set_pass`). Failed attempts and internal retries are not counted.
- **retrodoc-pipeline**: orchestrates the multi-pass generation pipeline of `PLAN.md` §2, one module per
  pass. The `//!` doc of each module is the reference for what the pass does and why; this is only the map.
  `generate` runs them in this order: surface (roles → glossary → entry points), actors, file budget, repo
  map, domains, features, use cases, diagrams, business-language score, confidence, then render.

  | Module | Role |
  |---|---|
  | `roles.rs` | One LLM call over the file tree + manifests: stack, glob rules `pattern -> role`, `source_extensions` (languages the walker doesn't know, promoted to `Source`), plus per-language `chunk_boundaries`. `roles.yaml` is hand-editable and reused unless `retrodoc roles --force`; `RoleRules::classify` applies it mechanically. |
  | `chunks.rs`, `chunk_check.rs` | Cut long files before a boundary regex (else a blank line); `Splitter::excerpt` for the use cases/confidence passes; `chunk_check` verifies the LLM's regexes on the real files and asks for a fix. |
  | `glossary.rs` | Entities from `model`-role files (LLM, batched by characters), test-block descriptions from `test`-role files (mechanical). |
  | `entry_points.rs` | Entry points (`http_route`, `cli_command`, `job`…) with their outputs, from `entrypoint`-role files. |
  | `surface.rs` | `Surface::new(glossary, entry_points)`: the capped prompt section that domain clustering starts from (business concepts, never layers). |
  | `actors.rs` | Business actors from authorization code + user-like entities; `retrodoc actors [--force]`. |
  | `ranking.rs` | Optional file budget (`--max-files` / `ingest.max_files`): role × churn × references; the rest becomes `FileKind::Other` (`scope.yaml`). |
  | `repo_map/` | File summaries (LLM + git history, cached by content hash, optionally batched/concurrent), then directory summaries bottom-up. |
  | `domains/` | One call clusters directory summaries + doc titles + surface into `DomainMap`; `coverage.rs` enforces 100% coverage by construction (uncategorized bucket, first assignment wins, hallucinated paths dropped). |
  | `features.rs` | One call per domain/sub-domain unit (not "uncategorized"), features grounded on a validated subset of its files. |
  | `use_cases/`, `slices.rs` | One call per feature with numbered code excerpts, entry points and the code they run (`CodeIndex::slice`); steps, actors, `narrative`, `primary_actor`; citations are validated. |
  | `diagrams.rs` | Deterministic Mermaid `sequenceDiagram` per use case, no LLM. |
  | `vocabulary.rs` | Deterministic `business_language` score per use case, recomputed each run. |
  | `confidence.rs` | LLM verdict per step against the cited code; score capped for ungrounded steps (`--no-confidence`, `--confidence-sample N`). |
  | `report.rs` | Documentation debt report from the saved artifacts (`retrodoc report`, no LLM). |

  Cross-cutting pieces: `batched_read.rs` (the loop of the glossary and entry points passes: files in
  batches of ~12k chars, an unusable batch retried file by file, a checkpoint after each batch), `artifact.rs` (load/save of everything under `.retrodoc/cache/`; a missing or
  unreadable file is a first run, not an error), `response.rs` (`complete_text`, and `complete_json` with
  lenient parsing and one retry; an unparseable unit is skipped with a warning), `fingerprints.rs` and
  `cache.rs` (incremental re-run), `progress.rs` (one `tracing::info!` line per unit with ETA), `naming.rs`
  (the single `normalize` used to match names across passes), `usage_log.rs` (the end-of-run recap text and the
  history of the last 20 runs in `.retrodoc/cache/usage.json`), `error.rs`.

  Behaviors that span passes:
  - **Incremental re-run**: each pass skips a unit whose input fingerprint is unchanged and reuses its saved
    result (features per domain unit, use cases per feature, summaries per file hash, directory summaries per
    listing hash, domains per clustering input). `generate --force` wipes the caches.
  - **Resumable**: an LLM failure in the features or use cases pass saves the units done so far; the repo map
    saves its cache. A rerun resumes there.
  - **Cost control**: `llm.concurrency` (default 1) parallelizes file and directory summaries,
    `llm.batch_chars` (default 6000, 0 = off) batches small files, and `generate` prints
    `estimate_repo_map`'s expected calls first.
  - **Token accounting**: `generate`, `roles`, `glossary`, `entry-points` and `actors` end with a recap of calls, tokens
    and time per pass (tokens only, no prices) and append the run to `.retrodoc/cache/usage.json`; kept apart from the
    rendered docs so reruns stay no-ops, and left alone by `generate --force`. `commands/usage.rs` builds the provider
    the five commands share.
  - `retrodoc_llm::HeartbeatProvider` (wrapped around the provider in the CLI) logs "still waiting for the
    LLM (Ns)" every 30 s, to tell a slow call from a stuck run.
- **retrodoc-render**: writes the Markdown/Mermaid output to the docs dir (`output.docs_dir`). `render()` builds
  the files in memory (deterministic), `plan()` compares them with the disk (created/updated + unified diff,
  stale files under `functional/` reported but never deleted), `WritePlan::apply()` writes only what differs.
  `_retrodoc/run-metadata.json` has a timestamp that is ignored when it's the only change, so reruns are no-ops.
  The CLI drops generated docs (`functional/`, `_retrodoc/`) from the ingested existing docs.
- **retrodoc-cli**: `clap` subcommands (`init`, `scan`, `generate`, `render`, `report`, `roles`, `glossary`,
  `entry-points`, `actors`, `surface`). `generate` runs the whole pipeline then writes the docs (`--dry-run` previews, `--force`
  ignores caches, `--no-confidence` / `--confidence-sample N` / `--max-files N` bound the cost, see the pipeline bullet); `render` writes/previews the docs from the cached artifacts (no LLM call); `report` prints
  the debt report (no LLM call); `roles`, `glossary`, `entry-points`, `actors` and `surface` are the standalone phase 7 commands above. `main` is `#[tokio::main]` since `generate` awaits the pipeline. `commands/workspace.rs` is the common start of the commands: `repo_root`, `Workspace::open` (root + `retrodoc.toml`), `Workspace::ingest`, `source_paths`.

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
  trait (see `CountingProvider` in `repo_map/tests.rs`) rather than hitting the network — follow that pattern for
  new pipeline passes instead of adding integration tests that need `OPENROUTER_API_KEY`.
- Filesystem-touching tests use `tempfile::tempdir()` and, where git history matters, actually run `git
  init`/`git commit` in the tempdir (see `git_history.rs` and `walker.rs` tests) rather than mocking `git2`.
