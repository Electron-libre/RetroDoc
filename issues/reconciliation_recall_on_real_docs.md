# Measure the reconciliation with the existing docs on a repository that has some

# Goal

The reconciliation of `issues/docs_reconciliation_debt_report.md` (spec `specs/docs-reconciliation.md`, ADR 0033)
is traced by tests with a fake LLM only. Know, on a repository whose docs really describe what it does, whether
the lexical match finds the sections that document an item, and whether the LLM judge tells covers, contradicts
and unrelated apart.

# Findings

* 2026-10-26, default smoke repo (a small Ruby library), local `qwen3.6:35b-a3b`: smoke conclusive, 23 of 23
  items undocumented, no section matched, no reconcile call. Its README has no functional text, so the verdict is
  right but the judge was not exercised.
* The match keeps a section that holds half of the content words of the name plus the description (decision 2 of
  the issue, the rule check it replaced). A feature description of two sentences has about 20 content words: the
  high review of the issue flagged that a section describing it in other words may never match.
  `Bm25::search_any` (any shared word) exists for long queries.

# Approach

1. Measure first, no code: on the public repository of the quality benchmark (its docs shown), run `generate`
   and count the items per status, then label by hand a sample of 20 items: does a doc section document it,
   which one.
2. Compare the recall of the current match with `search_any` plus a minimum BM25 score, and with the name alone
   as the query, on the same sample (a replay over the saved artifacts and docs, no LLM).
3. Read the verdicts of the judge on the matched items: false `covers` on a shared word, contradictions that
   are real, unparseable answers.
4. Change REC-2 only if a variant finds clearly more sections without making the judge's batches larger
   than the context of the local model.

# Specs

* `specs/docs-reconciliation.md`: REC-2 if the match changes.

# Resources

* `crates/retrodoc-pipeline/src/reconcile/` (`candidates`, `judge.rs`), `crates/retrodoc-pipeline/src/bm25.rs`
* `skill:quality-benchmark`, `benchmark/`
* ADR 0033, `issues/docs_reconciliation_debt_report.md` (Tracking: the high review)

# Hints

* A broader match costs judge calls: one batch per 8 matched items, the sections listed once per batch.
* The generated docs must stay out of the corpus (`project_docs`).
