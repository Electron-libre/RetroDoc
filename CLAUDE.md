# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

RetroDoc is a Rust CLI that uses LLM agents to catch up on a software project's documentation debt.
v1 (MVP) scope: **functional documentation only**, from a **local Git repo** (code + commit history +
existing Markdown docs), using **OpenRouter** or the **DeepSeek** API as LLM provider (same wire format). See `PRODUCT.md` for the long-term
product vision and the v1 scope, and `ROADMAP.md` for the milestones — read `ROADMAP.md` before starting new work,
it's the source of truth for what each milestone covers, its state and its issues; the "Scope of v1" of `PRODUCT.md`
says what's still out of scope for v1 (technical/C4 docs, GitHub/GitLab/Jira connectors, multi-provider LLM). The
dated findings behind the milestones are in `docs/findings.md`.

Commit messages follow the Conventional Commits format described in `.claude/skills/commit-message/conventional-commits.md`. Check `git log` to see
which milestones of `ROADMAP.md` are already done before assuming what's implemented; the released ones are moved to
`ROADMAP-done.md`.

## Index

Details live in separate files; read the one that matches the task:

- `docs/dev/commands.md`: build/test/lint commands, `just` recipes, measurement examples, registry note, CLI smoke test, API keys.
- `docs/dev/agent-harness.md`: hooks (`format-code`, `lint-gate`, `language-hint`, `review-gate`, run by `cardinal run`) and the skills in `.claude/skills/`. Most of it is synced by Cardinal (`cardinal sync`): never edit the copies (`just check-cardinal` fails), project values are in `.claude/cardinal.toml` and `.claude/update-docs-map.md`.
- `docs/dev/conventions.md`: codebase conventions (confidentiality of test repos, English only, clippy pedantic, test patterns).
- `specs/`: one spec per capability (numbered requirements, scenarios, non-goals), traced to tests by `// spec: ID`
  (ADR 0032). Read the spec of a capability before touching it; a change of behaviour starts by amending it
  (`skill:write-spec`).
- `docs/crates/`: per-crate detail. `docs/ARCHITECTURE.md`: high-level view. `docs/adr/`: decisions.
- `ROADMAP.md`: milestones (the released ones in `ROADMAP-done.md`); `PRODUCT.md`: vision and v1 scope; `docs/findings.md`: dated findings; `CHANGELOG.md`: releases
  (`just release`); `.claude/skills/commit-message/conventional-commits.md`: commit format.

Essentials: `cardinal run project-check` (`just check`: fmt + clippy -D warnings + tests + spec traceability) must pass before committing; it
needs the `cardinal` binary (`cargo install --git <source in .claude/cardinal.toml> cardinal --locked`); clippy is pedantic and warning-free; everything is in English; never name or quote
confidential smoke-test repos in committed files.

Measures (paid DeepSeek runs, hours-long local runs): `~/retrodoc-runs/LEDGER.md`, outside git, records each
measure (commit, model, inputs, cost, figures, artifacts), the runs to keep and the planned measures of the
queue. Read it before a paid or long run and reuse a measure of the same pass at the same code, model and
inputs; append a line after; still ask before a paid run. When an issue is created or the issues are
re-ranked (the `priority` and `depends_on` of each issue file), update its planned measures (skills `quality-benchmark` and `smoke-test` give the details).

## Architecture

Cargo workspace, ten crates under `crates/`, dependency direction flows one way (no cycles):

```
retrodoc-cli ──> retrodoc-pipeline ──> retrodoc-ingest ──> retrodoc-core
                                   ──> retrodoc-llm
             ──> retrodoc-llm      ──────────────────────> retrodoc-core
             ──> retrodoc-ingest
retrodoc-cli ──> retrodoc-render ──> retrodoc-core
retrodoc-cli ──> retrodoc-mcp ──> retrodoc-pipeline, retrodoc-ingest, retrodoc-core
retrodoc-cli ──> retrodoc-web ──> retrodoc-agent, retrodoc-llm, retrodoc-mcp, retrodoc-render, retrodoc-core
retrodoc-cli ──> retrodoc-bench ──> retrodoc-agent, retrodoc-mcp, retrodoc-pipeline, retrodoc-ingest, retrodoc-llm, retrodoc-core
retrodoc-cli ──> retrodoc-agent ──> retrodoc-mcp, retrodoc-llm
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
- **retrodoc-bench**: the measuring tools (ADR 0042): the quality benchmark (scoring, judge, replays of one pass on
  frozen inputs) and the measuring examples; reaches pass internals through the `bench` feature of the pipeline.
  → `docs/crates/bench.md`
- **retrodoc-render**: writes the Markdown/Mermaid docs, deterministic and idempotent. → `docs/crates/render-mcp-cli.md`
- **retrodoc-mcp**: the typed read model of the generated docs, the run history (a snapshot per `generate`), and
  the read-only, LLM-free MCP server over it.
  → `docs/crates/render-mcp-cli.md`
- **retrodoc-agent**: the agent of `ask` and `chat`, a bounded tool loop over the read-only tools of `retrodoc-mcp` (ADR 0047).
  → `docs/crates/agent.md`
- **retrodoc-web**: local web interface, HTML rendered on the server from the read model (ADR 0039), the ask page of `ui --ask` (ADR 0048, its steps streamed: ADR 0049).
  → `docs/crates/render-mcp-cli.md`
- **retrodoc-cli**: `clap` subcommands (`init`, `scan`, `generate`, `render`, `report`, `roles`, `brief`, `glossary`,
  `entry-points`, `business-read`, `business-files`, `contexts`, `actors`, `surface`, `benchmark`, `benchmark-table`, `ask`, `chat`, `search`,
  `mcp`, `ui`); logs go to stdout (stderr for `mcp`). → `docs/crates/render-mcp-cli.md`
