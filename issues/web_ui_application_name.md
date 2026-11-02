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

# Milestone

Web interface for a first-time reader

# Resources

* `crates/retrodoc-web/templates/base.html`, `crates/retrodoc-web/src/pages.rs` (`home`).
* `crates/retrodoc-pipeline/src/brief/`, `crates/retrodoc-core/src/config.rs`.
* ADR 0039, `docs/crates/render-mcp-cli.md`.

# Hints

* A brief changed by the new field invalidates the saved briefs (their `input_hash`): the fallback must keep a run
  done before the change readable.

# Tracking

Plan: validated

## Understanding
Name the documented application in the header, the titles and the home page of `retrodoc ui`, with the purpose of
the brief under the home heading. Means: a `[project] name` key of `retrodoc.toml`, else the repository directory;
no LLM field, no `origin` remote; the Markdown render and the MCP answers are out of scope (spec non-goal).

## Plan
1. [x] Application name in the web interface: `ProjectConfig` (`[project] name`) in `retrodoc-core`, the name and the
   brief purpose resolved in `retrodoc ui` and passed to `retrodoc-web`, shown in `base.html` (brand, title) and
   `home.html` (heading, purpose), the no-run page included; docs of the key; verified by router tests on the header,
   the title and the home page and a test of the fallback; makes true WEB-62, WEB-63, WEB-64. About 250 lines.

## Decisions
- Mode: autonomous
- Base commit: 9147beb4ecd620d713846250ef2953e2eb4b674c
- `init` does not write `[project] name`: the directory name is the default (answer 1 a).
- The purpose is shown whenever it is not empty, sources or not (answer 2 a).

## Summary of deliverable 1
- done: `[project] name` (`ProjectConfig::name_or_directory`), a `Project` passed to `retrodoc_web::router` and
  `serve`, the name in the header and the titles (`view::Shell`), the home page headed by it and the brief purpose.
- red: WEB-62, WEB-63, WEB-64: the tests did not compile (`retrodoc_web::Project` and `ProjectConfig` missing).
- verified on the saved healthchecks run: header and titles name `rsa-hc` (its directory), home headed by it and the brief purpose.
- follow-up: the Markdown render and the MCP answers don't carry the name (spec non-goal).

## High review of the issue (fixes as deliverable 2)
- Code review: 8 findings. Fixed: the wiring of `retrodoc ui` tested (`commands::ui::project`, WEB-62, WEB-64); an
  unreadable `product.yaml` warned of (`commands::brief::saved`); WEB-64 says "the saved product brief" (the
  `brief` command rewrites it without a run); the no-run page built by one helper; a style for `.purpose`; the crate
  doc says `serve`. Not fixed: a blank name inside `retrodoc-web` (the CLI always resolves one, the web crate shows
  what it is given) and a shared test helper for `Project::default()` (test-only churn).
- red: none (the fixes reuse the red of deliverable 1).
