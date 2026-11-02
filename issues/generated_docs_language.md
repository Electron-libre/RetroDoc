---
priority: medium
why: 'the docs are written in the language the model picks (English on the saved runs), so a team that reads another language cannot take them as they are; it touches every writing pass'
---
# Let the user choose the language of the generated documentation

# Goal

A `retrodoc.toml` key sets the language of the generated docs (names of the domains, features and use cases,
narratives, steps, rules, glossary definitions, brief), for example French. Without it the behaviour stays that of
today. The fixed text of the render and of the web interface (headings, labels) follows the same language, or the
issue says it does not.

# Findings

* `crates/retrodoc-core/src/config.rs` has no language key; no prompt of `crates/retrodoc-pipeline` names the output
  language (the brief asks for "business language", the register, not the tongue).
* The saved runs of public apps are written in English (healthchecks, linkding, lobsters); what a model writes for a
  repository whose comments and commits are in another language has not been measured.
* The render and the web interface have their fixed text in English (`<html lang="en">`, "Search the docs").

# Approach

1. Measure first: the language of the docs of a saved run whose code or commits are not in English (the large Rails
   app), to know whether the model already drifts.
2. An `[output] language` key (a name the model reads, as "French", or a BCP 47 tag mapped to one), added to every
   writing prompt as one instruction; the slugs and file names stay derived from the names, so check that the
   slugs of accented names stay stable.
3. The language is part of the input hash of each writing pass: changing it reruns them, keeping it is a no-op.
4. The scores that judge the docs (business language, confidence, the benchmark against an English reference) must
   still work on docs in another language: measure one pass at least.
5. The fixed text of the render and the interface: translated tables for a few languages, or left in English and said
   in the non-goals.

# Specs

* The config spec and the specs of the writing passes amended; `specs/web-ui.md` if the interface follows.

# Milestone

Docs in the reader's language

# Resources

* `crates/retrodoc-core/src/config.rs`, the prompts of `crates/retrodoc-pipeline/src/` (one module per pass).
* `docs/crates/pipeline.md` (input hashes, incremental re-run), `docs/crates/render-mcp-cli.md`.
* The measures ledger before any measure (paid runs: ask first).

# Hints

* `bm25.rs` already strips English and French stop words: the search keeps working on French docs.
* The glossary matches the names of the code (English identifiers) to the concepts: a translated concept name must
  keep its link to the identifier.

# Tracking

Plan: validated

## Understanding
`[output] language` makes the eleven writing passes write in that language through one instruction appended to
their system prompt by a wrapped provider (ADR 0041), reruns them when it changes, and keeps accented names'
slugs readable. Measure step 1 done (no LLM): the saved docs of the large private Rails app are in English (3,437
"the", 2 "des"), but its commit messages are mostly English too, so no drift to measure there.

## Plan
1. [x] Language setting: `[output] language` (names and two-letter tags), `init` line, the wrapping provider on the
   writing passes, the language in the print of the brief (the contexts map has none, reused until `--force`), accent-folding slugs, spec
   `docs-language.md` and ADR 0041; verified by config, provider, rerun and slug tests; makes true DLG-1 to DLG-6.
2. [x] Measure on delivery_router with DeepSeek in French (paid, cents): docs in French, entity names
   kept, rerun calls nothing, business-language and confidence scores against the English run of the same code;
   a fix of the business-language score if a French narrative loses its entity names; verified by the measure
   (ledger line) and a test of the fix if any.
3. [x] Non-goal of the render and web fixed text written, issue `localized_render_and_web_text` created; verified by
   `just check-specs` and the issue checker.

## Decisions
- Mode: autonomous
- Base commit: 6f8eab027e5c34b3d1595e016cf0dbfa3b84bcda
- Fixed text of the render and the web interface: stays in English, non-goal + its own issue (answer 1 a).
- The paid DeepSeek measure of deliverable 2 runs without another stop (answer 2 a).

## Deliverable 1: language setting

done: `[output] language` (name or tag), `init` comment line, `InLanguage` provider on the eleven writing passes,
`LlmProvider::language` in the brief print, accent-folding slugs, spec `docs-language.md` accepted, ADR 0041.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clippy pedantic clean |
| specs | DLG-1 to DLG-6 traced |
| docs | README, ARCHITECTURE §8, pipeline.md, llm.md |
| code review | low: no findings |

red: DLG-1, DLG-2, DLG-6: `no method named language_name found for struct OutputConfig`; DLG-3, DLG-4: `unresolved
import` of the language module, then the brief test panicked at `brief/tests.rs:208` (`calls() == 2`: the brief was reused);
DLG-5: `left: "gestion-des-contrats-lectroniques"`.

Code review: no findings (the only lead, trait methods InLanguage might hide, checked: all four are forwarded).

not verified: the binary test of DLG-2/DLG-3 was written after the code (not seen red); a real model's answers in
French (deliverable 2).

## Deliverable 2: measure in French, brief written again

done: `generate` writes again an unedited saved brief asked in another language (`ProductBrief.language`); measured
on delivery_router with deepseek-chat (ledger, 2026-11-02, `docs_language-6248f5d-{en,fr}`).

| figure | English | French |
|---|---|---|
| domains / features / use cases / rules | 3 / 6 / 8 / 24 | 4 / 9 / 17 / 50 (one run each) |
| confidence, business-language score | 96%, 100% | 87%, 99% |
| judged recall, precision (domains; features) | 100, 100; 100, 100 | 100, 75; 100, 89 |
| narratives rated business by the judge | 0 of 8 rated | 16 of 17 |
| rerun at the same language | no call | no call |
| reconciliation lines with the English README | 6 | 0 |

- Entity names keep the code's spelling (Customer, Order); actors and descriptions are in French; one attribute got a
  French gloss (`rider (livreur affecté)`).
- The scores work on French docs: no fix of the business-language score needed.
- Found: before the fix the brief stayed in English through `generate` (reused as it is), hence the fix.
- Found, out of this issue: the reconciliation finds no section of English project docs for French items
  (lexical match); the render leaves the pages of the English domains in `docs/` (an item renamed leaves its old
  page). Both become issues in deliverable 3.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clippy pedantic clean |
| specs | DLG-4 traced by the generate brief test too |
| docs | pipeline.md, ARCHITECTURE §4 |
| code review | low: no findings |

Code review: no findings

red: DLG-4: `no field language on type ProductBrief` (the generate test written before the field and the rewrite).

not verified: a repository whose code and commits are not in English; figures over several runs (one run per language).

## Deliverable 3: follow-up issues

done: the non-goal of `specs/docs-language.md` names `localized_render_and_web_text`; three issues created under the
milestone, each depending on this one: `localized_render_and_web_text` (low), `reconcile_across_languages`
(medium), `render_removes_pages_of_renamed_items` (low).

| check | result |
|---|---|
| tests | n/a (no code) |
| specs | `just check-specs` green |
| issues | `just check-issue` green, `just next-issue` lists them blocked by this issue |
| code review | low: no findings |

Code review: no findings

red: none
