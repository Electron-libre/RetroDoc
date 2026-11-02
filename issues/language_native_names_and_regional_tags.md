---
priority: low
depends_on: [localized_render_and_web_text]
why: 'the English name or the two-letter tag already works; a native name or a regional tag only loses the fixed text and the lang'
---
# Recognize a language written by its native name or a regional tag

# Goal

`[output] language` takes a name sent as written, or a two-letter tag of `LANGUAGE_TAGS` (DLG-2). The fixed text of
the docs and of the web pages, and the `lang` of the pages, are chosen from `OutputConfig::language_tag`, which only
knows the English names and the two-letter tags. A team that writes its language as it reads it gets French
narratives inside English headings, and pages with no `lang`. The tag should be found for the native names and the
regional tags of the languages `LANGUAGE_TAGS` names.

# Findings

* 2026-11-02, high review of `localized_render_and_web_text`: `language = "Français"`, `"fr-FR"` or `"fr_FR"` sends
  the name as written, so the writing passes answer in French, but `language_tag` gives `None`:
  `Texts::for_tag(None)` writes `## What happens` and `**Confidence:**`, and `Project::lang_of` gives no `lang`.

# Approach

1. Add the native name of each language to `LANGUAGE_TAGS` (`Français`, `Deutsch`, `Español`, `Italiano`,
   `Português`, `Nederlands`), matched ignoring case and accents.
2. Read a regional tag (`fr-FR`, `fr_FR`, `pt-BR`) by its first two letters, and decide whether the `lang` of the
   pages keeps the region.
3. Decide whether `language_name` should resolve these spellings to the English name too: that changes the prompts,
   so it redoes the writing passes of the runs that used them (DLG-4).

# Specs

* `specs/docs-language.md` (DLG-2, DLG-7), `specs/web-ui.md` (WEB-78).

# Milestone

Docs in the reader's language

# Resources

* `crates/retrodoc-core/src/config.rs` (`LANGUAGE_TAGS`, `language_name`, `language_tag`).
* `crates/retrodoc-render/src/texts.rs`, `crates/retrodoc-web/src/texts.rs`, `crates/retrodoc-web/src/lib.rs`
  (`Project::lang_of`).
* ADR 0041.

# Tracking

Plan: validated

## Understanding
`language_tag` also finds the tag from the native name (case and accents ignored) and from a regional tag by its
first two letters (DLG-9), so the fixed text and the `lang` follow. `language_name`, hence the prompts, stays as it
is: no pass is redone. Out of scope: the region in `lang`, non-Latin native names, more languages.

## Plan
1. [x] Native names and regional tags: `LANGUAGE_TAGS` gets the native name of each language, `language_tag` matches
   the tag, the English or native name with case and accents folded (the accent table of `slugify` moves to core and
   is shared), or the first two letters of `xx-YY`/`xx_YY`; verified by a core test (`pt-BR`, `Français`, `zh-TW`,
   prompts unchanged), the CLI French-docs test with `Français` and the web `lang_of` test with `fr-FR`; makes true
   DLG-9, DLG-7, WEB-78.
2. [ ] Docs and `high` review of the issue.

## Decisions
- Mode: autonomous
- Base commit: 7fa127c29b41b7c20e9d114e4545cba87d81557e
- The `lang` of the pages keeps the two letters only, no region.
- `language_name` is unchanged: a native name or a regional tag is sent as written, no pass is redone.

## Deliverable 1 summary
done: `language_tag` finds the tag from a native name (case and accents folded by `text::fold_latin`, the accent
table of `slugify` moved to core) or a regional tag; `language_name` unchanged.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (DLG accepted, DLG-9 traced) |
| docs | README sample, `docs/crates/render-mcp-cli.md` |
| code review | no findings |

red: DLG-9, DLG-7, WEB-78: `language_tag` of `Français` gave `None` instead of `Some("fr")`; the French-docs test found
`## Use cases`.

Code review: no findings

not verified: none

decisions needed: none
