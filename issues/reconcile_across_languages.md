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
