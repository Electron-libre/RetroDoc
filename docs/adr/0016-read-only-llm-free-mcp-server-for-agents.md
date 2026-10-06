# 0016. Serve the docs to agents through a read-only, LLM-free MCP server

Status: Accepted

## Context

Phase 9 ("ask the documentation", `PLAN.md` §7.3) planned an agent inside RetroDoc: tool calling added to
`retrodoc-llm`, a tool loop, then `ask` and `chat`. The first readers of the generated docs turned out to be
other agents, coding agents such as Claude Code or Cursor, which already have a model and a loop. What they
lack is a reliable way to query the docs without reading all of them or all the code. The docs are also not
always right: a confidence score says how well the code backs a claim, the code moves on after generation,
and the target repositories can be confidential.

## Decision

Serve the docs through a **local MCP server** (`retrodoc mcp --path <repo>`, stdio, the official `rmcp`
SDK) that is **read-only and calls no LLM**: no API key, no token cost, deterministic answers, and the calling
agent does the reasoning. The internal `ask`/`chat` and tool calling in `retrodoc-llm` are not needed for it;
they stay possible later. The code lives in a new crate, `retrodoc-mcp`, which reads the artifacts through
`retrodoc-pipeline`, the git history through `retrodoc-ingest`, and depends on nothing that calls a model.

- **Retrieval**: hierarchical navigation (domains, features, use cases, each answer giving the id to follow)
  plus a lexical BM25 search, written by hand, over the generated docs, the glossary and the collected
  Markdown docs. `tantivy` was rejected (a heavy dependency for a few hundred documents rebuilt at every start)
  and embeddings are not used until the lexical search is shown to fail on business vocabulary. A query loses
  its stop words (English and French) and an entry must match at least half of the rest.
- **Answers** are Markdown, with the id to reuse, the confidence (flagged below 50% with the advice to check the
  code) and the cited files. "Not documented" is a normal answer for an unknown id or an empty search, never
  a protocol error, and tells the agent not to guess.
- **Freshness**: at every call, the current hash of each cited file is compared with the one the last
  `generate` recorded in `repo-map.json`; modified and deleted files are named in a warning. No pipeline
  change was needed. The per-unit fingerprints could not be reused: they mix other inputs than file contents.
- **Code access is narrow**: `read_source` and `git_log` take only files the docs cite, never an id or an
  arbitrary path. They refuse absolute paths, `..`, any path that is or goes through a symbolic link, binary
  files and files over 1 MiB; reads are windowed by lines and long lines are cut; `git_log` shows hash, date and
  subject, without author names, and leaves merge commits out. They run off the async threads.
- **A static entry point** for agents that do not run the server: `functional/llms.txt`, produced by `render()`
  with the docs, in the fully generated area. An `AGENTS.md` section was rejected because it would write
  outside the docs directory.
- **stdout carries the protocol**, so the logs of `retrodoc mcp` go to stderr.

## Consequences

Any MCP-aware agent can navigate the docs, check a claim against the cited code and learn how it evolved,
without a key or a cost. Tried with a headless Claude Code agent on a small smoke repository: it found the
right use cases, reported the confidence, noticed a modified cited file and was refused the paths outside
the scope. Limits that remain, each tracked as an issue:

- The relevance filter is lexical: with two words one match is enough, and synonyms are bridged by the agent
  reformulating, which costs calls (`issues/mcp_server.md`).
- After a `generate` that failed once the repo map was done, the cache is ahead of the docs and a changed file
  looks fresh (`issues/stale_docs_after_failed_generate.md`); and every answer re-reads the cited files
  (`issues/mcp_freshness_cost.md`).
- An agent cannot check what the docs do not cite, and `git_log` does not follow renames
  (`issues/git_log_follow_renames.md`).
- The server speaks MCP only over stdio, with no network authentication; HTTP is left for later.
