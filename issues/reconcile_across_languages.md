---
priority: medium
depends_on: [generated_docs_language]
why: 'with docs in another language than the project''s own, the reconciliation silently finds nothing, so the debt report loses its contradicted and missing sections'
---
# Reconcile generated docs with project docs written in another language

# Goal

The reconciliation matches the sections of the project's own docs to the generated items (BM25, then a judge).
With `[output] language` set to another language than the project docs, no section matches and the reconciliation
is empty without a word. It should still find the sections that speak of an item, or say why it can't.

# Findings

* 2026-11-02, delivery_router, deepseek-chat (ledger, `docs_language-6248f5d-{en,fr}`): `reconciliation.yaml` holds
  6 lines in English, 0 in French; the French run logs no reconciliation judgement at all.
* The match is lexical (`crates/retrodoc-pipeline/src/reconcile/mod.rs`, `Bm25`): French item names share no word
  with the English README. The entity names, kept in the code's spelling, are the only common words.

# Approach

Measure first on the saved runs (no LLM): which words the items and the sections share in each language.

1. Index the items with the identifiers they cite (entity names, entry point names, file paths) besides their text,
   so the shared code words match across languages.
2. Index the English names the reading passes keep (entities, entry points) next to the translated text.
3. At least a warning when the language is set and no section matched while the project has docs.

# Specs

* `specs/docs-reconciliation.md`.

# Milestone

Docs in the reader's language

# Resources

* `crates/retrodoc-pipeline/src/reconcile/`, `crates/retrodoc-pipeline/src/bm25.rs`.
* `specs/docs-reconciliation.md`, ADR 0033, ADR 0041.
* The measures ledger (runs `docs_language-6248f5d-en` and `-fr`).

# Tracking

Plan: validated

## Understanding
Match the items to the project docs written in another language through what both share: the code's identifiers
(entry point names, entity names), as a third query after the text and the name (REC-2); warn when the docs language
is set and nothing matched (REC-16). Out of scope: translating the queries, semantic retrieval, the judge's prompt.

## Measure (2026-11-02, no LLM, `~/retrodoc-runs/reconcile-replay` mode `xlang`)
* linkding shown run (English), 16 labelled documented items + 4 undocumented: text queries (`candidates()`) recall
  13/16, false matches 3/4. Identifiers alone (entry point names after `#`, a feature also its use cases'), the worst
  case of a translation that shares no word: recall 13/16, false 2/4. With file stems added: 11/16, false 3/4 (noise of
  `models`, `forms`, `utils`): stems are not kept.
* Identifiers as the third query on the English run: recall 13/16, false 3/4 unchanged; 6 of 330 items newly matched.
* delivery_router: 5 of 41 items newly matched in English, 4 of 80 in French (the README holds 19 words).
* One query of all the identifiers matches no domain of linkding (half of dozens of route words); each identifier
  searched alone, the hits ranked by their best score: features and use cases unchanged (13/16, false 2/4), 2 of 5
  domains matched. Kept: each identifier is its own query.

## Plan
1. [x] Identifiers query: `Item` carries its identifiers (entry point names; a domain: its features'; entity names of a
   domain or a rule), each searched alone when the text and the name match nothing; verified by tests of `candidates()` (a French
   item matched to an English section by its entry point, an English item unchanged); makes true REC-2.
2. [ ] Warning: `generate` warns when the docs language is set, the corpus is not empty and no candidate has a section;
   verified by a test of the warning; makes true REC-16; the spec back to `accepted`.

## Decisions
- Mode: autonomous
- Base commit: 19127660dd51426edd28b266b83c56b452eb3d43

## Deliverable 1 (identifiers query)
done: `Item::identifiers` (entry point names, a domain its features'; entities of a domain or a rule), each searched
alone when the text and the name match nothing, sections ranked by their best score.
red: REC-2: `classement/archivage: the name of its entry point finds it`, left `[]`, right `["README.md#Bookmarks"]`.
Code review: no findings.
