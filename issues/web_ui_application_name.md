---
priority: medium
why: 'every page of the interface names RetroDoc, never the documented application: a reader who keeps several runs open cannot tell them apart, and the fix is small'
---
# Show the name of the documented application in the web interface

# Goal

The web interface names the application it documents: in the header, in the `<title>` of every page and on the home
page, with the purpose of the product brief under it. Today a reader sees "RetroDoc" and the domains, never which
application they belong to.

# Findings

* 2026-11-02, `retrodoc ui` on a saved run of a public Django app (healthchecks), deepseek-flash run of 2026-10-27:
  the header shows `RetroDoc`, Files, Glossary, Rules, Actors; the titles are `<page> · RetroDoc`; the home page lists
  the domains with no heading naming the application.
* `crates/retrodoc-web/templates/base.html` hard-codes `RetroDoc` in `<title>` and in the `brand` link.
* The product brief (`.retrodoc/cache/product.yaml`) has a `purpose` that names the application in its text
  ("Healthchecks is a service for monitoring…") but no field holding the name alone.
* `retrodoc.toml` has no key for a project name.

# Approach

Where the name comes from, candidates (can be combined, in this order of precedence):

1. A `[project] name` key in `retrodoc.toml`, written by `retrodoc init`.
2. A `name` field of the product brief, asked of the brief pass (it already reads the README).
3. The name of the repository directory, or of the `origin` remote, as the fallback with no LLM.

Then the interface: the name in the header and the titles (`<page> · <name>`), the purpose of the brief on the home
page; the render (`llms.txt`, the index Markdown) and the MCP read model can take the same name.

# Specs

* `specs/web-ui.md` amended (new requirement after WEB-61); the specs of the brief or the config if the name comes
  from them.

# Resources

* `crates/retrodoc-web/templates/base.html`, `crates/retrodoc-web/src/pages.rs` (`home`).
* `crates/retrodoc-pipeline/src/brief/`, `crates/retrodoc-core/src/config.rs`.
* ADR 0039, `docs/crates/render-mcp-cli.md`.

# Hints

* A brief changed by the new field invalidates the saved briefs (their `input_hash`): the fallback must keep a run
  done before the change readable.
