# Serve the documentation to LLM agents through an MCP server

# Goal

Let coding agents (Claude Code, Cursor, …) query the functional documentation RetroDoc generates, to
understand how an application works without reading all its code. Delivered as a local MCP server, plus a
static entry point for agents that don't run it.

# Approach

The consuming agent is the LLM, so the server needs none: it is deterministic, read-only, with no API key and
no token cost. This replaces the internal agent of `PLAN.md` §7.3 as the first way to ask the documentation
(`retrodoc ask` / `chat` stay possible later) and makes the tool-calling extension of `retrodoc-llm`
unnecessary for this feature.

Retrieval: hierarchical navigation (domains → features → use cases → cited files) plus a BM25 lexical index
over the generated artifacts and the collected Markdown docs. No embeddings: only if the lexical search
fails on business vocabulary that differs from the code's, measured on a smoke-test repo.

Tools (names indicative, read-only): `list_domains`, `get_domain`, `get_feature`, `get_use_case`,
`search_docs`, then `read_source` and `git_log`. Every answer cites its sources and carries the confidence
score; it warns when the cited files changed since generation (fingerprints); it says "not documented"
rather than guessing.

# Decisions

* Transport: stdio first (`retrodoc mcp --path <repo>`); HTTP later if needed.
* Official Rust SDK `rmcp`, not a hand-written protocol.
* First version serves the generated docs only; code access (`read_source`, `git_log`) comes in a second
  delivery, limited to the repo root and to the files the docs already cite.
* Also generate a static agent entry point (`llms.txt`-style index or `AGENTS.md` section pointing to
  `docs/functional/`), useful without the server.
* Out of scope: writing to the target repo, embeddings, internal `ask`/`chat`, network authentication.

# Resources

* `PLAN.md` §7.3 (phase 9), `PRODUCT.md` "Ask the documentation"
* `.retrodoc/cache/` artifacts (`Artifact` in `crates/retrodoc-pipeline/src/artifact.rs`), `report.rs`
  (builds from saved artifacts with no LLM: same pattern), `fingerprints.rs`
* `crates/retrodoc-render` (layout of the generated docs)

# Hints

* Never name confidential test repos in committed files.
* Test the tools directly against artifacts built in a `tempdir()`; no LLM, so no fake provider is needed.

# Tracking

1. [ ] Index and search: BM25 over the generated artifacts (domains, features, use cases) and the collected
   Markdown docs, exposed by the debug command `retrodoc search "<query>"` (no LLM). Decide the crate
   (`retrodoc-mcp` or `retrodoc-agent`). Verified by tests on artifacts in a `tempdir()`, then retrieval
   quality judged on a smoke-test repo.
2. [ ] MCP server over stdio: `retrodoc mcp --path <repo>` with `list_domains`, `get_domain`, `get_feature`,
   `get_use_case`, `search_docs`. Answers cite their sources, carry the confidence and say "not documented"
   when nothing is found. Verified by tool tests and an end-to-end test with an MCP client (`initialize`,
   `tools/list`, `tools/call`).
3. [ ] Freshness: answers warn when the cited files changed since generation (fingerprints). Verified by a
   test that modifies a file after generation.
4. [ ] Code and git tools: `read_source` and `git_log`, limited to the repo root and to the files cited by the
   docs. Verified on a temporary git repo, including attempts to leave the scope (`../`).
5. [ ] Static agent index: `generate` writes an `llms.txt`-style index or an `AGENTS.md` section in the docs,
   deterministic so reruns stay no-ops. Verified by an idempotent render test. Can be moved before 3 and 4.
6. [ ] ADR (MCP server, LLM-free, lexical retrieval) and `docs/ARCHITECTURE.md` update; other docs updated
   with each deliverable through `update-docs`.

## Decisions

* Transport: stdio first; HTTP later if needed.
* Official `rmcp` SDK.
* First version serves the generated docs only; code access in deliverable 4.
* Static agent entry point generated with the docs (deliverable 5).
* `PLAN.md` §7.3 and `PRODUCT.md` updated on 2026-10-06 to reflect the MCP approach.
