# Which documents to update after a change

Read by `skill:update-docs`. Owned by the project: one row per kind of change, with the documents that describe it.

| Change | Update |
|---|---|
| New/renamed/removed CLI command or flag | `docs/dev/commands.md`, `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md` §3 (commands) |
| New pipeline pass or changed pass behavior | `docs/crates/pipeline.md`, `docs/ARCHITECTURE.md` §4 (pipeline, safety nets, confidence) and §5 (incremental re-run) if caching changed, `docs/ARCHITECTURE.md` §4 |
| New artifact or cache file under `.retrodoc/` | `docs/crates/`, `docs/ARCHITECTURE.md` §7 (what lands on disk) |
| Output files written to the docs dir | `docs/ARCHITECTURE.md` §6, `docs/crates/render-mcp-cli.md` |
| New/changed `retrodoc.toml` key | `retrodoc-core` doc comments in `config.rs`, the default file written by `Config::write_default` (used by `retrodoc init`), any doc showing a sample config |
| New crate, moved module, changed dependency direction | `CLAUDE.md` architecture diagram + crate map, `docs/crates/`, `docs/ARCHITECTURE.md` §2, `commit_scopes` in `.claude/cardinal.toml` (new crate) |
| LLM provider / boundary change | `docs/ARCHITECTURE.md` §8, `docs/crates/llm.md`, `PRODUCT.md` "Scope of v1" and "Risks" if v1 limits move |
| Milestone started, finished, or re-scoped | `ROADMAP.md` (its state and issues; `skill:create-issue` and `skill:issue-workflow` keep them), the matching `docs/findings.md`.x section when findings are logged there, `CLAUDE.md` if it lists what is implemented |
| New or changed convention (lint, tests, language) | `docs/dev/conventions.md` |
| Product-level scope change (new connector, doc type) | `PRODUCT.md` ("Capabilities", "Scope of v1") |
| Public Rust API added or changed | doc comments: `# Errors` section on `pub fn` returning `Result`, `#[must_use]` on pure getters (clippy pedantic) |
