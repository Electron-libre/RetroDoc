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

## Index

Details live in separate files; read the one that matches the task:

- `docs/dev/commands.md`: build/test/lint commands, `just` recipes, measurement examples, registry note, CLI smoke test, API keys.
- `docs/dev/agent-harness.md`: hooks (`format-rust`, `clippy-gate`, `review-gate`, run by `harness run`) and the skills in `.claude/skills/`. Most of it is synced by `harness sync`: never edit the copies (`just check-harness` fails), project values are in `.claude/harness.toml` and `.claude/update-docs-map.md`.
- `docs/dev/conventions.md`: codebase conventions (confidentiality of test repos, English only, clippy pedantic, test patterns).
- `specs/`: one spec per capability (numbered requirements, scenarios, non-goals), traced to tests by `// spec: ID`
  (ADR 0032). Read the spec of a capability before touching it; a change of behaviour starts by amending it
  (`skill:write-spec`).
- `docs/crates/`: per-crate detail. `docs/ARCHITECTURE.md`: high-level view. `docs/adr/`: decisions.
- `PLAN.md`: roadmap; `PRODUCT.md`: vision; `AGENTS.md`: commit format.

Essentials: `just check` (fmt + clippy -D warnings + tests + spec traceability) must pass before committing; it
needs the `harness` binary (`cargo install --git <source in .claude/harness.toml> harness --locked`); clippy is pedantic and warning-free; everything is in English; never name or quote
confidential smoke-test repos in committed files.

Measures (paid DeepSeek runs, hours-long local runs): `~/retrodoc-runs/LEDGER.md`, outside git, records each
measure (commit, model, inputs, cost, figures, artifacts), the runs to keep and the planned measures of the
queue. Read it before a paid or long run and reuse a measure of the same pass at the same code, model and
inputs; append a line after; still ask before a paid run. When an issue is created or `issues/queue.md` is
re-ranked, update its planned measures (skills `quality-benchmark` and `smoke-test` give the details).

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
  `entry-points`, `business-read`, `business-files`, `contexts`, `actors`, `surface`, `benchmark`, `benchmark-table`, `search`,
  `mcp`); logs go to stdout (stderr for `mcp`). → `docs/crates/render-mcp-cli.md`
