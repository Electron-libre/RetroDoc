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

- **retrodoc-core**: shared domain model (`Domain`/`Feature`/`UseCase`/`Step`/`ConfidenceScore` in
  `model.rs` — populated starting the "domains" phase, still empty in v1 so far) and `retrodoc.toml`
  config (`config.rs`).
- **retrodoc-ingest**: reads the target repo — file walker respecting `.gitignore` (`walker.rs`; classifies files as `Source`/`Test`/`Markdown`/`Other` —
  tests are detected by directory or file-name convention and kept out of the pipeline, which only
  consumes `Source`), one-pass
  git history per file via `git2` revwalk+diff (`git_history.rs`, not a `git log` per file — matters for
  perf on large repos; `file_log` is the exception, the recent commits of one file for the MCP `git_log`), existing Markdown docs (`existing_docs.rs`), and the signals of the product brief (`signals.rs`: `Signal` with a citable origin; doc sections cut at headings, root `*.md` included; commit subjects from `git_history::scan_history`, the same one revwalk as `collect_history`, without merges, bots and dependency bumps, `feat`/`fix` first in a Conventional Commits repo; root manifest metadata, the tree two levels deep, Gherkin `.feature` titles and test descriptions (`test_phrases`, shared with the glossary), schema tables, migration names and translated texts (one language), read by format (SQL DDL, `ActiveRecord` schema, YAML/JSON/properties/gettext) from the files a `SourceMap` selects (rules `kind + glob + format`; `SourceMap::sniff` guesses them from file content on any stack, no location is hard-coded; `retrodoc-pipeline`'s `sources` replaces it with an LLM-inferred, editable one); `signals::collect(repo_root, &ingest, &sources)` gathers them all, with `IngestResult.commits` coming from the same walk as the history; ADR 0019). `run()` combines the three into an
  `IngestResult`.
- **retrodoc-llm** (`types.rs`, `heartbeat.rs`, `openrouter.rs`, `usage.rs`): `LlmProvider` trait abstraction (kept provider-agnostic) + `OpenRouterProvider`, which serves
  `provider = "openrouter"` and `"deepseek"` (same chat-completions format, own endpoint and key variable; ADR 0024), a real `reqwest` HTTP client with exponential-backoff retry on
  429/5xx that honors the delay the server asks for (`Retry-After`, Google's `retryDelay`; up to 120 s, a longer
  one is an error). Per-request HTTP timeout is 120 s unless `llm.timeout_secs` is set (a slow local
  model writing a long JSON answer needs more; a timeout restarts the whole generation on retry). `CompletionRequest.json_schema` is the optional shape of a JSON answer, sent as a strict `response_format` (plus OpenRouter's `require_parameters` on its real endpoint) unless `llm.structured_output = false`; a 400/404/422 is sent again without it, and if that works the schema is left out for the rest of the run (ADR 0023). `CompletionResponse.usage`
  is the optional token count the server reports (`usage.rs`: a missing or malformed block is `None`, never estimated);
  `UsageProvider` + `UsageTracker` count answered calls and tokens per pass and per model (concurrency-safe; the CLI names
  the pass with `set_pass`, or `in_pass`, which closes it when the work ends). Failed attempts and internal retries are not counted.
- **retrodoc-pipeline**: orchestrates the multi-pass generation pipeline of `PLAN.md` §2, one module per
  pass. The `//!` doc of each module is the reference for what the pass does and why; this is only the map.
  `generate` runs them in this order: surface (roles → glossary → entry points), actors, file budget, repo
  map, domains, features, use cases, diagrams, business-language score, confidence, then render.

  | Module | Role |
  |---|---|
  | `roles/` | One LLM call over the file tree + manifests: stack, glob rules `pattern -> role`, `source_extensions` (languages the walker doesn't know, promoted to `Source`), plus per-language `chunk_boundaries`. `roles.yaml` is hand-editable and reused unless `retrodoc roles --force`; `RoleRules::classify` applies it mechanically. |
  | `sources/` | Where the schema, migrations and translations are, and their format (`retrodoc_ingest::SourceMap`): `SourceMap::sniff` (content, no LLM), then one LLM call over tree + manifests + that guess, rules checked against the real files (`check_rule`; a bad rule is sent back once, then dropped), sniffed sources the LLM missed kept. `signal-sources.yaml` is hand-editable: edited (rules hash differs) it is kept, unedited it is inferred again only when the tree shape changes or with `force`. An LLM failure leaves the sniffed map. |
  | `brief/` | The product brief (ADR 0019): one LLM call over a bounded sample of the signals of `retrodoc_ingest::signals` (`sample.rs`: budget per kind, ids `S1`.. the LLM cites, resolved to origins; unknown citations dropped, a claim with none reads as unsupported) writes purpose, users, objects, capabilities, external systems and open questions. `product.yaml` is hand-editable: edited (content hash differs) it is kept, untouched it is reused while the sample is the same (commits left out of that hash), `force` writes it again. `ProductBrief::prompt_section` / `fingerprint` are what the later passes will read. `generate` runs first: the saved brief as it is, or one written now when there is none (`retrodoc brief [--force]` is how it is refreshed). It heads the prompts of `roles` (not in its reuse: the saved rules stay), `glossary`, `entry_points`, `actors`, `domains`, `features` and `use_cases` (its fingerprint is part of their reuse, per unit for the last two; an empty brief changes nothing). `Evidence` (`evidence.rs`), only with `brief.evidence = true` in `retrodoc.toml` (off by default, benchmarks showed fewer features and no gain: `issues/brief_injection_experiments.md`), is what `generate` also keeps of the signals: per feature (use cases) or domain unit (features), the doc sections, test descriptions and commit subjects whose words are closest to the unit's names, description and files (`Bm25::search_any`, about 1,200 / 900 / 900 characters), put in its prompt and in its fingerprint, so a new commit about a unit redoes that unit alone. The standalone commands read the saved brief. |
  | `business_files/` | Where the business lives (`issues/locate_business_files.md`): one LLM call over the tree, the stack, the brief and cheap evidence (per directory: files, commits, most changed names; doc titles; test vocabulary) answers a ranked, bounded list of files and directories with a reason (`BusinessMap`, paths that are no source dropped). `business-files.yaml` is hand-editable: edited (content hash differs) it is kept, untouched it is reused while the tree shape and the brief are the same, `force` asks again. `BusinessMap::files` expands it to at most 60 source files. `generate` runs it right after the roles (stack known) and before the glossary; `retrodoc business-files [--force]` is how it is refreshed. The glossary reads it (`build_glossary`'s `business` list, at most 60 files) instead of the `model` role. |
  | `chunks.rs`, `chunk_check.rs` | Cut long files before a boundary regex (else a blank line); `Splitter::excerpt` for the use cases/confidence passes; `chunk_check` verifies the LLM's regexes on the real files and asks for a fix. |
  | `glossary/` | Entities from the business files (LLM, batched by characters; without a business list, from the `model`-role files, ADR 0025), test-block descriptions from `test`-role files (mechanical). |
  | `entry_points/` | Entry points (`http_route`, `cli_command`, `job`…) with their outputs, from `entrypoint`-role files. |
  | `surface.rs` | `Surface::new(glossary, entry_points)`: the capped prompt section that domain clustering starts from (business concepts, never layers); `missing_entities_notice` is the `info` line `generate` logs when source files exist but no entity was found. |
  | `actors.rs` | Business actors from authorization code + the glossary entities (the LLM keeps those that stand for someone); `retrodoc actors [--force]`. |
  | `ranking.rs` | Optional file budget (`--max-files` / `ingest.max_files`): role × churn × references; the rest becomes `FileKind::Other` (`scope.yaml`). |
  | `repo_map/` | File summaries (LLM + git history, cached by content hash, optionally batched/concurrent), then directory summaries bottom-up. |
  | `domains/` | One call clusters directory summaries + doc titles + surface into `DomainMap`; `repair.rs` places the files it left unassigned with one small extra call; `coverage.rs` enforces 100% coverage by construction (uncategorized bucket, first assignment wins, hallucinated paths dropped). |
  | `features/` | One call per domain/sub-domain unit (not "uncategorized"), features grounded on a validated subset of its files. |
  | `use_cases/`, `slices.rs` | One call per feature with numbered code excerpts, entry points and the code they run (`CodeIndex::slice`); steps, actors, `narrative`, `primary_actor`; citations are validated. |
  | `diagrams.rs` | Deterministic Mermaid `sequenceDiagram` per use case, no LLM. |
  | `vocabulary.rs` | Deterministic `business_language` score per use case, recomputed each run. |
  | `confidence/` | LLM verdict per step against the cited code; score capped for ungrounded steps (`--no-confidence`, `--confidence-sample N`). |
  | `report.rs` | Documentation debt report from the saved artifacts (`retrodoc report`, no LLM). |
  | `benchmark/` | Quality benchmark (`issues/done/quality_benchmark.md`, `issues/done/benchmark_spread_and_judge_bias.md`): `Reference` (hand-written `benchmark/<repo>/reference.yaml`, checked on load) and `RunMetrics` (sizes, mean business-language score and the share of use cases at score 1, mean confidence, cost of the last `generate`, read from `.retrodoc/cache/`, no LLM). `Matches` (hand-written `matches.yaml`, wins over the normalized-name match) and `compare` (recall and precision of domains and features, with what is left unmatched). `judge` (LLM proposals for what is left unmatched, rating of narratives; never overrides `matches.yaml`) and `summary` (the text `retrodoc benchmark` prints) and `table` (`RunReport` per run, series per configuration, mean/range/change table). |

  Cross-cutting pieces: `batched_read.rs` (the loop of the glossary and entry points passes: files in
  batches of ~12k chars, an unusable batch retried file by file, a checkpoint after each batch), `artifact.rs` (`Artifact`: the names of everything under `.retrodoc/cache/` and which `generate --force` clears; load/save of those files; a missing or
  unreadable file is a first run, not an error), `response.rs` (`complete_text`, and `complete_json` with
  the `schemars` schema of its answer type sent as `response_format`, `response_schema` making it strict — every raw answer type derives `JsonSchema`; lenient parsing (fences, prose, a repeated field keeps its last value) and one retry; an unparseable unit is skipped with a warning), `fingerprints.rs` and
  `cache.rs` (incremental re-run), `progress.rs` (one `tracing::info!` line per unit with ETA), `naming.rs`
  (the single `normalize` used to match names across passes), `usage_log.rs` (the end-of-run recap text and the
  history of the last 20 runs in `.retrodoc/cache/usage.json`), `error.rs`.

  Behaviors that span passes:
  - **Incremental re-run**: each pass skips a unit whose input fingerprint is unchanged and reuses its saved
    result (features per domain unit, use cases per feature, summaries per file hash, directory summaries per
    listing hash, domains per clustering input; a feature answered "no use case" twice cleanly is remembered too). `generate --force` wipes the caches.
  - **Resumable**: an LLM failure in the features or use cases pass saves the units done so far; the repo map
    saves its cache. A rerun resumes there.
  - **Cost control**: `llm.concurrency` (default 1) parallelizes file and directory summaries,
    `llm.batch_chars` (default 6000, 0 = off) batches small files, and `generate` prints
    `estimate_repo_map`'s expected calls first.
  - **Token accounting**: `generate`, `roles`, `glossary`, `entry-points` and `actors` end with a recap of calls, tokens
    and time per pass, plus the answers a pass could not parse and the units it skipped (`LlmProvider::note_unparseable_answer`, sent by `complete_json`; tokens only, no prices) and append the run to `.retrodoc/cache/usage.json`; kept apart from the
    rendered docs so reruns stay no-ops, and left alone by `generate --force`. `commands/usage.rs` builds the provider
    the five commands share.
  - `retrodoc_llm::HeartbeatProvider` (wrapped around the provider in the CLI) logs "still waiting for the
    LLM (Ns)" every 30 s, to tell a slow call from a stuck run.
- **retrodoc-render**: writes the Markdown/Mermaid output to the docs dir (`output.docs_dir`). `render()` builds
  the files in memory (deterministic), `plan()` compares them with the disk (created/updated + unified diff,
  stale files under `functional/` reported but never deleted), `WritePlan::apply()` writes only what differs.
  `_retrodoc/run-metadata.json` has a timestamp that is ignored when it's the only change, so reruns are no-ops. `functional/llms.txt` (`agent_index`) is the `llms.txt`-style entry point for agents that don't
  run the MCP server: domains and features with links and confidence, how to read them and a pointer to `retrodoc mcp`.
  The CLI drops generated docs (`functional/`, `_retrodoc/`) from the ingested existing docs.
- **retrodoc-mcp** (`corpus.rs`, `search.rs`, `freshness.rs`, `source.rs`, `tools.rs`, `server.rs`): read-only access to the generated docs
  for LLM agents, with no LLM call (see `issues/mcp_server.md`). `SearchIndex` ranks (the hand-written BM25 of `retrodoc-pipeline`'s `bm25.rs`, shared with the pipeline, title counted
  three times, stop words in English and French dropped, at least half of the query words must match) one `Entry` per
  domain, sub-domain, feature, use case, glossary concept and collected Markdown doc, each with its id, confidence and
  cited files. `Docs` (`Docs::load` reads `.retrodoc/cache/` + the collected docs) answers the seven tools in Markdown
  (`list_domains`, `get_domain`, `get_feature`, `get_use_case`, `search_docs`, `read_source`, `git_log`): ids to reuse, confidence (flagged below
  50%), cited files, a stale warning when a cited file no longer matches the hash `generate` recorded in `repo-map.json`
  (`Freshness`, checked at every call; files without a recorded hash are not judged), and "Not documented" as a normal answer for an unknown id or an empty search. `server.rs` puts them
  behind the `rmcp` SDK (`serve_stdio`); tested end to end with an `rmcp` client over an in-memory pipe. `source.rs` (`SourceAccess`) backs `read_source` (a
  window of lines, 200 by default, 400 at most) and `git_log` (hash, date, subject, no author): only files the docs
  cite, never an absolute path, a `..`, a path that is or goes through a symlink (even inside the repo), a binary or a
  file over 1 MiB; a line over 400 characters is cut. `git_log` leaves merge commits out. The two tools run in
  `spawn_blocking`.
- **retrodoc-cli**: `clap` subcommands (`init`, `scan`, `generate`, `render`, `report`, `roles`, `brief`, `glossary`,
  `entry-points`, `actors`, `surface`, `benchmark`, `benchmark-table`, `search`, `mcp`). `generate` runs the whole pipeline then writes the docs (`--dry-run` previews, `--force`
  ignores caches, `--no-confidence` / `--confidence-sample N` / `--max-files N` bound the cost, see the pipeline bullet); `render` writes/previews the docs from the cached artifacts (no LLM call); `report` prints
  the debt report (no LLM call); `business-files [--force]` locates where the business lives (`business-files.yaml`); `brief [--force] [--signals]` locates the schema and translations (`signal-sources.yaml`), then writes and prints the product brief with where each claim comes from (`--signals`: only how much evidence there is per kind and the sample size, no LLM call); `roles`, `glossary`, `entry-points`, `actors` and `surface` are the standalone phase 7 commands above; `benchmark --reference <file> [--matches <file>] [--judge] [--out <json>]` prints the figures of the last run and its recall/precision against a hand-written reference (no LLM call; `--judge` asks the model for pairs and a narrative rating, saved in `.retrodoc/benchmark/judge.yaml`, outside the cache so `generate --force` leaves it; `--out` saves the figures as JSON); `benchmark-table <dir> [--previous <dir>]` tabulates saved runs (mean, range, change; no LLM call); `search "<query>"` prints the best matches of the lexical search and `mcp` serves the docs to an agent over stdio (neither calls the LLM; both need a `generate` run). `main` is `#[tokio::main]` since `generate` awaits the pipeline. Logs (`tracing`) go to stdout, in color only on a terminal: a redirected log is plain text; `mcp` logs to stderr because its stdout carries the protocol. `commands/workspace.rs` is the common start of the commands: `repo_root`, `Workspace::open` (root + `retrodoc.toml`), `Workspace::ingest`, `source_paths`.

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
