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

1. [x] Index and search: BM25 over the generated artifacts (domains, features, use cases) and the collected
   Markdown docs, exposed by the debug command `retrodoc search "<query>"` (no LLM). Decide the crate
   (`retrodoc-mcp` or `retrodoc-agent`). Verified by tests on artifacts in a `tempdir()`, then retrieval
   quality judged on a smoke-test repo.
   Smoke test on `delivery_router` (small Ruby repo, 18 entries): navigation and cited files work, a
   business question finds the right use case. Limits seen, for deliverable 2: no synonym matching
   ("driver" vs "rider"), and no relevance threshold, so "cancel a delivery" (undocumented) still returns
   entries sharing "delivery"; "not documented" only fires when no word matches. Consider stop words and
   requiring most query words to match. The smoke verdict was FAIL on two pipeline warnings unrelated to
   this deliverable (6 files left uncategorized, one feature answered "no use case" once).
2. [x] MCP server over stdio: `retrodoc mcp --path <repo>` with `list_domains`, `get_domain`, `get_feature`,
   `get_use_case`, `search_docs`. Answers cite their sources, carry the confidence and say "not documented"
   when nothing is found. Verified by tool tests and an end-to-end test with an MCP client (`initialize`,
   `tools/list`, `tools/call`).
   Tested end to end with an `rmcp` client over an in-memory pipe and with the real binary on stdin/stdout.
   Tried by a headless Claude Code agent (file tools disabled) on the `delivery_router` smoke repo: tools
   discovered, answers cite confidence and files, an undocumented topic (cancellation) is reported as such.
   Limits seen: with two-word queries one common word is enough to match, so the agent gets off-topic
   hits and needs extra searches (10 to 13 calls on open questions); synonyms are bridged by the agent, not
   by the index. Idea if calls matter: show the steps of the use cases in `get_feature`.
3. [x] Freshness: answers warn when the cited files changed since generation (fingerprints). Verified by a
   test that modifies a file after generation.
   Tried by a headless Claude Code agent after editing a cited file of the `delivery_router` smoke repo: it
   reported the docs as stale. Follow-ups: `issues/stale_docs_after_failed_generate.md` (false "fresh" after a
   `generate` that failed past the repo map) and `issues/mcp_freshness_cost.md` (files re-read at every call).
4. [x] Code and git tools: `read_source` and `git_log`, limited to the repo root and to the files cited by the
   docs. Verified on a temporary git repo, including attempts to leave the scope (`../`).
   Verified on temporary git repos (scope escapes: `..`, absolute path, symlinks, uncited file, binary, large
   file, long line, merge commits) and by a headless Claude Code agent on the `delivery_router` smoke repo,
   which read the cited code and history and got the exact refusals. Limits: an agent cannot check what the
   docs do not cite (it wanted the rider code that holds the real calculation); renames are not followed
   (`issues/git_log_follow_renames.md`); the history walk is not bounded on a large repo.
5. [x] Static agent index: `generate` writes an `llms.txt`-style index or an `AGENTS.md` section in the docs,
   deterministic so reruns stay no-ops. Verified by an idempotent render test. Can be moved before 3 and 4.
   Written as `functional/llms.txt` (decision: the fully generated area, no `AGENTS.md` section since it would
   live outside the docs dir). Lists domains and features with links and confidence, how to read them and a
   pointer to `retrodoc mcp`; no use cases, to stay short. Not tried with an agent that has no server: it has to
   be told where the file is.
6. [x] ADR (MCP server, LLM-free, lexical retrieval) and `docs/ARCHITECTURE.md` update; other docs updated
   with each deliverable through `update-docs`.
   ADR `0016` written, with a new section 9 in `docs/ARCHITECTURE.md` (diagram of the server); the other docs
   were updated with each deliverable. Follow-ups, each its own issue: `issues/stale_docs_after_failed_generate.md`,
   `issues/mcp_freshness_cost.md`, `issues/git_log_follow_renames.md`.

## Decisions

* Transport: stdio first; HTTP later if needed.
* Official `rmcp` SDK.
* First version serves the generated docs only; code access in deliverable 4.
* Static agent entry point generated with the docs (deliverable 5).
* `PLAN.md` §7.3 and `PRODUCT.md` updated on 2026-10-06 to reflect the MCP approach.
* Crate `retrodoc-mcp` (index and, later, server); BM25 written by hand, no `tantivy` (small corpus, no dependency).
* Indexed corpus (deliverable 1): domains, features, use cases, collected Markdown docs and the glossary.
  Entry points and actors left out for now.
* Relevance (deliverable 2): drop stop words (English and French) from the query and require at least half
  of the remaining words (rounded up) to match an entry, for `search_docs` and `retrodoc search`. Known
  limit: with two words, one match is enough, so "cancel a delivery" still returns entries sharing "delivery".
* Tool answers are readable Markdown, not JSON (the reader is an LLM); an unknown id answers "not documented"
  as a normal result, not a protocol error.
* Logs go to stderr for `retrodoc mcp`: stdout carries the protocol.
* Freshness (deliverable 3): compare the current hash of each cited file with the one in `repo-map.json` (written
  by the last `generate`), at every call; no pipeline change. Names the modified and deleted files; files
  without a recorded hash (docs, skipped files) are not judged. Known limit: after a `generate` that failed
  once the repo map was done, the cache is ahead of the docs and a changed file looks fresh until the next
  successful `generate`.
* Code access (deliverable 4): `read_source(path, start_line?, end_line?)` and `git_log(path, limit?)` take
  only files cited by the docs (a feature's source paths or a step's source refs), never an id. `git_log` shows
  short hash, date and subject, no author name. Refused: absolute paths, `..`, anything that resolves
  outside the repo root (symlinks included), binary and very large files. Reads are windowed by lines.

