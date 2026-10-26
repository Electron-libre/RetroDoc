# Measure the reconciliation with the existing docs on a repository that has some

# Goal

The reconciliation of `issues/done/docs_reconciliation_debt_report.md` (spec `specs/docs-reconciliation.md`, ADR 0033)
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
* 2026-10-26, linkding at the pinned commit, series `shown`, `deepseek-chat`, rules off (benchmark default), run in
  `~/retrodoc-runs/retrodoc-benchmark-linkding/clones-1793046053/shown-1`: corpus of 113 sections (README, `docs/`),
  330 items (5 domains, 61 features, 264 use cases). 119 matched (36%), 15 reconcile calls, 21 s. Report:
  51 documented, 0 matched-not-judged, 278 undocumented (211 with no section, 67 judged unrelated), 1 contradicted.
* Hand labels, 20 items (12 features, 8 use cases; 16 documented by a section, 4 not), in
  `~/retrodoc-runs/reconcile-replay/labels.yaml`: the current match puts a labelled section in its top 3 for
  **8 of 16** documented items, and matches none of the 4 undocumented ones. Missed: read/unread, tags, bundles,
  REST API, sign-in (OIDC, auth proxy), superuser provisioning, API tokens, bundles of a user. Tags and bundles are
  matched, but to `api.md#Bookmarks` (4,700 chars, shares most words), not to `api.md#Tags` / `#Bundles`.
* The top 11 of the report's undocumented list (most entry points): 9 are documented (API tags, bundles, assets,
  tokens twice, archive, HTML snapshots twice, snapshot queue), the current match finds **0 of 9**. The head of
  the list the reader acts on is mostly wrong.

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
* ADR 0033, `issues/done/docs_reconciliation_debt_report.md` (Tracking: the high review)

# Hints

* A broader match costs judge calls: one batch per 8 matched items, the sections listed once per batch.
* The generated docs must stay out of the corpus (`project_docs`).

# Tracking

Plan: validated

## Understanding
Measure, on linkding (its `docs/src/content/docs/*.md` describe its features), the recall of the REC-2 match and the
quality of the judge's verdicts, against a hand-labelled sample of 20 items. Out of scope: semantic retrieval, judging
the generated items themselves. REC-2 changes only if a variant clearly wins (deliverable 4, conditional).

## Plan
1. [x] Shown run: `generate` on linkding at the pinned commit with its docs in the corpus; count the items per status
   and label 20 items by hand (documented by which section, or not); verified by the counts and the label table in
   `# Findings`.
2. [ ] Match replay: recall of the current match, of `search_any` with a minimum BM25 score, and of the name alone,
   on the 20 labelled items, from the run's artifacts and docs, no LLM; verified by a recall / sections-per-item /
   judge-batches table in `# Findings`.
3. [ ] Judge audit: read the verdicts on the matched items (false `covers`, real contradictions, unparsed answers);
   verified by the counts in `# Findings`.
4. [ ] (only if a variant of 2 clearly wins) Amend REC-2, test, code; verified by `just check` and a replay of 2
   with the new match; makes true REC-2.

## Decisions
- Mode: autonomous
- Base commit: 46033390274ee801899852d8850323e28488242b
- 1a: the shown run uses hosted DeepSeek (`--provider deepseek`).
- 2a: the replay tool is a throwaway Rust example, not committed; only the figures go in the issue.
- 3a: the run goes through `just benchmark --series shown`, 1 run.

## Deliverable 1

done: shown run on linkding (DeepSeek), status counts and 20 hand labels in `# Findings`.

| check | result |
|---|---|
| run | `generate` ok, 3 WARN schema refused (DeepSeek, expected), 1 WARN grounding (use cases) |
| tests, lint, specs, docs | n/a (no code) |
| code review | n/a (diff under `issues/` only) |

red: none. Not verified: `retrodoc report` bails ("no confidence scores found") on a `--no-confidence` run; the
reconciliation was read from `docs/_retrodoc/coverage-report.md` and `.retrodoc/cache/reconciliation.yaml`.
