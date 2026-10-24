# Which documents to update after a change

Read by `skill:update-docs`. Owned by the project: one row per kind of change, with the documents that describe it.

| Change | Update |
|---|---|
| New/renamed/removed CLI command or flag | `docs/dev/commands.md`, `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md` §3 (commands), `PLAN.md` §4 (`retrodoc-cli` list) |
| New pipeline pass or changed pass behavior | `docs/crates/pipeline.md`, `docs/ARCHITECTURE.md` §4 (pipeline, safety nets, confidence) and §5 (incremental re-run) if caching changed, `PLAN.md` §2 |
| New artifact or cache file under `.retrodoc/` | `docs/crates/`, `docs/ARCHITECTURE.md` §7 (what lands on disk) |
| Output files written to the docs dir | `docs/ARCHITECTURE.md` §6, `PLAN.md` §3 (expected output), `docs/crates/render-mcp-cli.md` |
| New/changed `retrodoc.toml` key | `retrodoc-core` doc comments in `config.rs`, the default file written by `Config::write_default` (used by `retrodoc init`), any doc showing a sample config |
| New crate, moved module, changed dependency direction | `CLAUDE.md` architecture diagram + crate map, `docs/crates/`, `docs/ARCHITECTURE.md` §2, `PLAN.md` §4, `commit_scopes` in `.claude/harness.toml` (new crate) |
| LLM provider / boundary change | `docs/ARCHITECTURE.md` §8, `docs/crates/llm.md`, `PLAN.md` scope and risks if v1 limits move |
| Roadmap phase started, finished, or re-scoped | `PLAN.md` §5 (remove "Not started" / add status) and the matching §7.x section, `CLAUDE.md` if it lists what is implemented |
| New or changed convention (lint, tests, language) | `docs/dev/conventions.md` |
| Product-level scope change (new connector, doc type) | `PRODUCT.md`, `PLAN.md` §1 scope |
| Public Rust API added or changed | doc comments: `# Errors` section on `pub fn` returning `Result`, `#[must_use]` on pure getters (clippy pedantic) |
