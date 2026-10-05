# 0001. Rust workspace with one-way crate dependencies

Status: Accepted

_Retroactive ADR, reconstructed from the history (9b15c37, 2026-09-28; 087eecd for the lints)._

## Context

RetroDoc ingests a repository, calls an LLM through several passes and writes Markdown. Those concerns
change at different speeds (the LLM provider, the pipeline passes, the output format) and the pipeline
must be testable without network or filesystem output.

## Decision

A Cargo workspace of six crates under `crates/`: `core` (domain model, config), `ingest` (walker, git
history, existing docs), `llm` (provider trait and implementations), `pipeline` (passes), `render`
(Markdown/Mermaid writing) and `cli` (`clap` binary). Dependencies flow one way, with no cycle: `cli`
depends on everything, `pipeline` on `ingest`, `llm` and `core`, `render` only on `core`.
`clippy::pedantic` is enabled workspace-wide (`[workspace.lints.clippy]`), each crate opting in with
`[lints] workspace = true`. The alternative, a single crate with modules, was rejected because it lets
the pipeline reach into rendering or HTTP details and makes the provider swap harder.

## Consequences

Pipeline code is tested against a fake `LlmProvider`, and rendering against in-memory data. Adding a
crate is a deliberate act. Pedantic lints add small costs (`# Errors` doc sections, `#[must_use]`) and
the build must stay warning-free (enforced later by the clippy gate hook, see 0014).
