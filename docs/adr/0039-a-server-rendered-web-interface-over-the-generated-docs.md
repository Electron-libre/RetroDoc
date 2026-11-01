# 0039. A server-rendered web interface over the generated docs

Status: Accepted

## Context

The results of a run are Markdown pages under `docs/functional/`, the debt report `docs/_retrodoc/coverage-report.md`,
the artifacts of `.retrodoc/cache/` and the MCP server for agents (ADR 0016). A human who takes an application over
has no structured way to browse them: the Markdown pages are read file by file, the debt report is one long page, and
nothing shows where the debt concentrates, what changed since the last run or what to fix first. The users want to
explore the docs, explore the debt to find the parts that suffer, read an audit dashboard to decide where to improve
the docs, and later review and edit the docs.

The interface runs on the user's machine in v1, but it is meant to be hosted for a team later (`PRODUCT.md`): the
choices made now must not prevent it. The project is a Rust workspace checked by `just check` (fmt, clippy pedantic,
tests), installed with `cargo install`; a second toolchain (Node, a WebAssembly target) would be a new cost for every
contributor and every install.

## Decision

A new command, `retrodoc ui --path <repo> [--bind <addr>]`, serves a web interface from a new crate, `retrodoc-web`:
an HTTP server (`axum`) that renders HTML pages on the server from templates checked at compile time (`askama`), with
`htmx` for the partial updates (filters, search, later the review and edit forms). The static assets (`htmx`,
`mermaid` to draw the diagrams, the stylesheet) are vendored in the crate and embedded in the binary: no CDN, so the
interface works offline and a confidential repository leaks nothing to a third party. It calls no LLM and, until the
review and edit milestones, writes nothing.

The crate reads the docs through the loaders `retrodoc-mcp` already uses (cache artifacts, collected docs, search
index, freshness, bounded source access), exposed as typed data rather than the Markdown answers of the MCP tools,
and depends on `retrodoc-mcp`, `retrodoc-render` (the rules of what a page shows), `retrodoc-pipeline` and
`retrodoc-core`; `retrodoc-cli` depends on it. The pages are built from a read model independent of HTTP, so a later
hosted mode (several repositories, authentication) or another front end reuses it. The server binds to `127.0.0.1` by default.

Rejected: **Leptos** with hydration needs the `wasm32` target and `cargo-leptos`, two builds and a heavier install,
for pages that are mostly read; Leptos rendered on the server only would bring a reactive framework for what
templates and `htmx` do. A **static HTML site** written by `render` was rejected because review and editing need a
server; it stays possible as an export. A **terminal interface** cannot draw the diagrams nor be shared.

## Consequences

The docs, the debt and the figures of a run can be browsed in a browser with the binary alone. The interactions stay
simple (links, forms, fragments swapped by `htmx`); a highly interactive view (a graph to drag, an editor with live
preview) will need a small dedicated script or a new decision. The vendored assets must be updated by hand. Follow-ups:
the read model has to be extracted from the private fields of `retrodoc_mcp::Docs`; the run history needs a snapshot of
the debt report per run, which the pipeline does not keep yet; editing needs the pipeline to keep a human change on
rerun (a new ADR); hosting needs authentication and several repositories (a new ADR).
