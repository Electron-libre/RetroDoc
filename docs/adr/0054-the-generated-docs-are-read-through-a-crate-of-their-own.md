# 0054. The generated docs are read through a crate of their own

Status: Proposed

## Context

`retrodoc-mcp` grew from the MCP server of [ADR 0016](0016-read-only-llm-free-mcp-server-for-agents.md) into the
reader of the generated docs: the typed read model, the lexical search, the code access scope, the stores of the
human reviews and edits ([ADR 0044](0044-the-human-reviews-live-in-one-file-outside-the-cache.md),
[ADR 0045](0045-the-human-edits-are-an-overlay-outside-the-cache.md)), the run history
([ADR 0043](0043-the-run-history-is-one-snapshot-file-per-run.md)), the audit, the debt view, and the read-only
tools. The web interface ([ADR 0039](0039-a-server-rendered-web-interface-over-the-generated-docs.md)), the `ask`
agent ([ADR 0047](0047-the-ask-agent-is-a-crate-over-the-read-only-tools.md)), the benchmark and the CLI depended
on it for the docs, and so on `rmcp`, for a server only `retrodoc mcp` starts. Since the readers' types, loaders and
computations moved to `retrodoc-core` (ADRs 0051, 0053), the crate no longer depends on the pipeline, so the reading
of the docs can stand alone.

## Decision

A `retrodoc-docs` crate holds every module about the generated docs: the read model, the corpus and the search,
the freshness of the cited files, the source access, the stores of the reviews and edits, the debt, the audit, the
run history, the report inputs, and the read-only tools with their catalog (names, descriptions, argument schemas,
call by name). The tools stay there and not in the server because the `ask` agent offers the very same tools as the
MCP server, with the same answers: the catalog is a reader of the docs, not a part of the protocol.
`retrodoc-mcp` keeps only the MCP server (`rmcp`, stdio), which wires the catalog to the protocol. Every reader
(web, agent, benchmark, CLI) depends on `retrodoc-docs`; only the CLI depends on `retrodoc-mcp`.

Rejected: keeping the tools in `retrodoc-mcp`, which would leave the agent, the web interface and the benchmark
depending on the MCP crate for the wording of the answers; a third crate for the tools alone, one more crate for
about 1,400 lines that every reader of them also needs the read model for.

## Consequences

- An export or a hosted reader depends on `retrodoc-docs` alone, without `rmcp`.
- `retrodoc-docs` still uses the pipeline in its tests, to write a map of the contexts as `generate` saves it.
- The crate docs of the read model move from `docs/crates/render-mcp-cli.md` to `docs/crates/docs.md`.
