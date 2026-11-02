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

# Tracking

Plan: validated

## Understanding
The fixed text of the Markdown render and of the web pages follows `[output] language`: a French table beside the
English one, chosen from the resolved tag; any other language keeps English. `<html lang>` from the tag. Out of
scope: the coverage report, the CLI output, the MCP tools, paths and anchors. The MCP and web read models read the
cache artifacts, never the rendered Markdown, so no parser depends on the headings.

## Plan
1. [x] Render: `OutputConfig::language_tag()` in core, a `Texts` table (English, French) in `retrodoc-render` passed
   to every page, the CLI passes the one of the config; verified by render tests in French and German and a
   re-render that writes nothing; makes true DLG-7, DLG-8. (~450 lines)
2. [x] Web layout: the language reaches `App`, `<html lang>`, a web texts table for the layout, nav, home, search,
   empty and not-found pages; verified by router tests; makes true WEB-78 and starts WEB-77. (~500 lines)
3. [x] Web item pages: the domain, feature, use case, rules, glossary, actors, map, files and code pages use the
   table; verified by a test that no English label is left on each page in French; makes true WEB-77. (~600 lines)
4. [x] Fixes of the `high` review of the whole issue; verified by `just check` and the tests below.

## Decisions
- Mode: autonomous
- Base commit: 40fb1715566c4c6819be0029588f6e2ce40e78fe
- A language name with no tag (`Japanese`) gives no `lang` attribute rather than a wrong `en`.
- `llms.txt` and `rules.md` follow the language like the other pages.

## Deliverable 1

done: `Texts` table (English, French) in `retrodoc-render`, threaded through every page, `llms.txt` and `rules.md`;
`OutputConfig::language_tag()`; the CLI picks the table from the config.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | DLG-7, DLG-8 traced by CLI tests |
| docs | `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md` §8, `README.md` |
| code review | no findings |

red: DLG-7, DLG-8: `## Use cases left in # Functional documentation`

Code review: no findings

- The web spec amendment (WEB-77, WEB-78) is held back to deliverable 2, so `just check-specs` stays green.
- The web keeps `Texts::ENGLISH` until deliverables 2 and 3.

## Deliverable 2

done: `WebTexts` table (English, French) in `retrodoc-web`, given to every template as `t`; `Project::lang` from
`Project::lang_of`, `<html lang>`; the frame, home, search, no-run and not-found pages, the confidence badge and the
reference pages in the table.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | WEB-77 (frame and these pages), WEB-78 traced in `crates/retrodoc-web/tests/pages.rs` |
| docs | `docs/crates/render-mcp-cli.md`, `README.md` |
| code review | no findings |

red: WEB-77: `Search the docs left in <!doctype html><html lang="fr">` (checked by removing the French entry of
`WebTexts::for_tag`; the first run of the new tests failed to compile, `Project` had no `lang`); WEB-78: same compile
failure.

Code review: no findings

- WEB-77 says the credit line of WEB-69 stays as it is (a brand line).
- A count of 0 reads `0 fonctionnalités` in French (the singular is only for 1, as in English).

## Deliverable 3

done: the domain, feature, use case, rules, glossary, actors, map, files, code and collected doc pages and the macros
take their fixed text from `WebTexts`; the kinds of a rule, an evidence and a citer stay enums the templates name; the
breadcrumb macro writes the home link; the refusals of a cited file have a French wording (`MAX_FILE_BYTES` made public
in `retrodoc-mcp` for it).

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | WEB-77 traced on every page in `crates/retrodoc-web/tests/item_pages.rs` |
| docs | `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md` §8 |
| code review | no findings |

red: WEB-77: `Map of this domain left in <main>`

Code review: no findings

- No English text node is left in the templates but the credit line and the commands.

## Deliverable 4: fixes of the high review

done: the colon after a citer and after a shared domain comes from the table (`WebTexts::colon`); French counts take
the singular for 0 (`WebTexts::count`); the page titles and the map crumb read `WebTexts::page_name(href)`, not an
index; the search headings reuse the words of the docs table and the glossary page name; the domain and code pages
call a `home_crumb` macro and lose their always-empty `crumbs`; a test guards that the web table of a tag shares the
docs table of that tag.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | no change |
| docs | none needed |
| code review | no findings |

red: none (fixes; the WEB-29 test now checks `Feature:` before the link)

Code review: no findings

High review: 10 findings.
1. Citer colon dropped: fixed.
2. French plural of 0: fixed.
3. Native names and regional tags (`Français`, `fr-FR`) get English fixed text: outside DLG-2/DLG-7, issue
   `issues/done/language_native_names_and_regional_tags.md`.
4. `ui` or `render` after a language change but before `generate` mixes the new fixed text with the old generated
   text: not fixed, the config is the source of the language and the next `generate` redoes the writing passes (DLG-4).
5. Hard-coded colon on the map page: fixed.
6. Reference pages read by index: fixed.
7. Two `for_tag` matches: kept (one per crate), guarded by a test.
8. `t` on every template rather than on `Shell`: kept, the htmx fragments have no `Shell` and one name keeps the
   macros working in both.
9. Dead `crumbs` fields: fixed.
10. Duplicated strings: `search_features` and `search_glossary` removed; `existing_doc` and `states_otherwise` kept
    (other forms, without the colon).
