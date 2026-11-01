---
priority: high
why: 'the first step of the web interface: every other page of the milestone is served by this crate and command'
---
# Serve the domains of the generated docs in a local web interface

# Goal

The results of a run can only be read as Markdown files, one at a time. Add the `retrodoc ui` command and the
`retrodoc-web` crate of ADR 0039: a local HTTP server that renders HTML pages on the server, with the layout every
later page shares (navigation, the tree of domains, features and use cases, the confidence and freshness badges) and
a first page that lists the domains of the last run with their description, confidence and counts of features and use
cases. A repository with no run yet gets a page that says to run `generate`, not an error.

# Approach

1. Extract from `retrodoc_mcp::Docs` (private fields, Markdown answers) a typed read model the web crate can use:
   the loaded domains, features, use cases, rules, freshness and search index, through accessors or a loader shared
   with `Docs::load`. The MCP answers must not change.
2. The `retrodoc-web` crate: `axum` router, `askama` templates (base layout, home page, not-found page, no-run page),
   the vendored `htmx` and stylesheet embedded in the binary and served under one path.
3. The `retrodoc ui --path <repo> [--bind <addr>]` subcommand: `127.0.0.1` and a fixed default port, the URL printed at
   start, logs to stdout like the other commands.
4. Tests on the router without a socket (`tower::ServiceExt::oneshot`) over the fixtures the MCP tests use: the home
   page lists the domains with their confidence, the tree links to the features, an unknown path is a 404 page, a repo
   without cache gets the no-run page.

# Specs

* A new `specs/web-ui.md` (the command, the bind address, the pages served, the no-run case), written at
  `issue-workflow` step 1.

# Milestone

Browse the docs in a web interface

# Resources

* `docs/adr/0039-a-server-rendered-web-interface-over-the-generated-docs.md`
* `docs/adr/0016-read-only-llm-free-mcp-server-for-agents.md`
* `crates/retrodoc-mcp/src/tools/mod.rs` (`Docs`, `Docs::load`), `crates/retrodoc-mcp/src/freshness.rs`
* `crates/retrodoc-cli/src/commands/mcp.rs` (how a read-only command opens the workspace)
* `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md`

# Hints

* The new crate changes the dependency graph of `CLAUDE.md` and `docs/ARCHITECTURE.md`, and needs a `web` scope in
  `commit_scopes` of `.claude/cardinal.toml`; link ADR 0039 from `docs/ARCHITECTURE.md` and set it `Accepted`.
* Escape everything that comes from the docs: names and narratives are LLM output and may hold HTML.
* Keep the read model independent of HTTP (ADR 0039): a later hosted mode reuses it.
