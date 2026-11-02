---
priority: low
depends_on: [generated_docs_language]
why: 'the generated text is already in the chosen language; the fixed headings in English are a comfort gap, not a blocker'
---
# Write the fixed text of the docs and the web interface in the language of the docs

# Goal

With `[output] language` set, the narratives, steps, rules and glossary are written in that language, but the fixed
text around them stays English: the Markdown headings and labels of the render, and the web interface. A French
team reads "What happens", "Confidence", "Business rules" around French text. The fixed text should follow the
language for the few languages `[output] language` names by tag, and stay English for any other.

# Findings

* 2026-11-02, delivery_router, deepseek-chat, `language = "fr"`: the use case pages read `## What happens`,
  `**Confidence:**`, `**Business language:**`, `**Triggered by:**`, `## Business rules` around French text
  (`crates/retrodoc-render/src/markdown/use_case.rs`).
* The web interface has `<html lang="en">` in `crates/retrodoc-web/templates/base.html` and English labels
  ("Search the docs").
* `specs/docs-language.md` lists this as a non-goal of the first issue.

# Approach

1. Gather the fixed strings of `retrodoc-render` and `retrodoc-web` into one table per language (English, then
   French), chosen from the tag `OutputConfig::language_name` resolved; a language with no table keeps English.
2. `<html lang>` from the tag.
3. Check the render stays deterministic and idempotent, and that the MCP and the web read model do not parse the
   headings it writes.

# Specs

* `specs/docs-language.md` (the non-goal becomes a requirement), `specs/web-ui.md`.

# Milestone

Docs in the reader's language

# Resources

* `crates/retrodoc-render/src/markdown/`, `crates/retrodoc-web/templates/`, `crates/retrodoc-core/src/config.rs`
  (`LANGUAGE_TAGS`).
* `docs/crates/render-mcp-cli.md`, ADR 0041.
