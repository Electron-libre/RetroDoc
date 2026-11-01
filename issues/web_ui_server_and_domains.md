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

# Tracking

Plan: validated

## Understanding
`retrodoc ui` serves, from a new `retrodoc-web` crate (axum + askama, htmx and stylesheet embedded), the shared
layout (tree, confidence and stale badges) and the home page of the domains; no-run and 404 pages. Out of scope: the
domain, feature and use case pages (next issue: the tree links answer 404 until then), search, glossary, code.

## Plan
1. [x] Read model: a typed `GeneratedDocs` in `retrodoc-mcp` (domains, features, use cases, rules, glossary,
   freshness, search index; `load` and accessors, no HTTP), `Docs` built on it; verified by the MCP tests unchanged
   (pure refactor, ~300 lines).
2. [x] `retrodoc-web` crate: router, base layout with the tree, home, not-found and no-run pages, assets under
   `/assets/`; verified by router tests (`oneshot`) over the MCP fixtures; makes true WEB-4 to WEB-12 (~700 lines,
   htmx vendored minified).
3. [x] `retrodoc ui --path [--bind]` subcommand, `web` commit scope, docs (CLAUDE.md and ARCHITECTURE.md graphs,
   `docs/crates/render-mcp-cli.md`, README), ADR 0039 `Accepted`; verified by a binary test on `--bind 127.0.0.1:0`
   (URL printed, home page fetched) and a parse test of the default; makes true WEB-1 to WEB-3 (~250 lines).

## Decisions
- Mode: autonomous
- Base commit: a736a458df0afebd44c7bf23f8bfe2c4486a87f8
- The docs are read once at start (no reload per request); default bind `127.0.0.1:8730`.
- Routes of the later pages, linked by the tree: `/domains/{domain}`, `/domains/{domain}/{feature}`,
  `/domains/{domain}/{feature}/{use_case}`.

## Deliverable 1 summary
- done: `retrodoc_mcp::GeneratedDocs` (`read_model.rs`) holds the loaded artifacts, glossary, search index and
  freshness with accessors and `features_of` / `use_cases_of`; `Docs` wraps it with the source access.
- red: none (pure refactor, the MCP tests unchanged but one that set a private field, now through a test-only
  `features_mut`).
- Code review: 1 findings: `features_of` / `use_cases_of` unused while the tools repeated the filters; fixed, the
  tools call them.

## Deliverable 2 summary
- done: crate `retrodoc-web` (router, `view.rs`, base layout with the tree, home, not-found and no-run pages,
  `htmx` 2.0.11 and the stylesheet embedded under `/assets/`), `web` commit scope, crate docs and graphs.
- red: WEB-4, WEB-5, WEB-6, WEB-7, WEB-8, WEB-9, WEB-10, WEB-11: the empty router answered 404 with no body
  (`assertion left == right failed: left: 404, right: 200`); WEB-12 passed on the empty router already.
- Escaping: askama writes numeric entities (`&#60;`), the WEB-10 test checks that form.
- Code review: no findings.

## Deliverable 3 summary
- done: `retrodoc ui --path [--bind]` (default `127.0.0.1:8730`, URL printed once it listens, the docs read once),
  docs (README, CLAUDE.md, ARCHITECTURE, crate docs, commands), ADR 0039 `Accepted`, `specs/web-ui.md` `accepted`.
- red: WEB-2, WEB-3, WEB-9: `error: unrecognized subcommand 'ui'` (no URL printed); WEB-1 written with the command.
- Code review: no findings.
