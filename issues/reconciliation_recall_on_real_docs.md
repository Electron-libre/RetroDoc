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
* Replay of the match (no LLM, throwaway tool in `~/retrodoc-runs/reconcile-replay`, not committed), on the 20
  labelled items plus the top 11 (25 documented, 6 undocumented), top 3 kept. Recall: a labelled section in the
  top 3; false: an undocumented item matched; batches: one per 8 matched items over the 330.

  | variant | recall | false | items matched | sections | batches |
  |---|---|---|---|---|---|
  | current: half of the words, name + description | 8/25 | 1/6 | 119 | 196 | 15 |
  | half of the words of the name only | 18/25 | 5/6 | 284 | 740 | 36 |
  | current, else half of the name when nothing matches | 18/25 | 5/6 | 285 | 604 | 36 |
  | a third of the words, name + description | 16/25 | 4/6 | 238 | 544 | 30 |
  | 40% of the words, name + description | 14/25 | 4/6 | 194 | 408 | 25 |
  | `search_any`, name + description, BM25 >= 10 | 16/25 | 4/6 | 278 | 640 | 35 |
  | `search_any`, name + description, BM25 >= 15 | 10/25 | 4/6 | 147 | 233 | 19 |
  | `search_any`, name only | 21/25 | 5/6 | 325 | 968 | 41 |

  An absolute BM25 floor is a bad knob: the score grows with the length of the query, so it keeps the long
  queries and drops the short ones. Every wider variant matches most undocumented items too, so whether it
  helps rests on the judge answering `unrelated` (deliverable 3). The context of a batch does not grow with any
  variant: at most 8 items x 3 sections x 1,500 chars (`MAX_SECTION_CHARS`), sections listed once.
* Judge audit, the 196 answers of the run: 78 `covers`, 117 `unrelated`, 1 `contradicts`, none unparsed or
  missing (0 items left matched).
  * `covers`: about 12 of 78 are false, the section only shares words or a field name: tags, bundles and favicons
    "covered" by `api.md#Bookmarks` (a `tag_names` field, a `bundle` filter), the UI bulk edit and archived list by
    the admin app's section, "create a bookmark" by the two browser-extension sections of `archiving.md`, a favicon
    by `backups.md`, password sign-in by `LD_ENABLE_OIDC`. The item's status is often right anyway (another
    section covers it) but the linked section is wrong.
  * `unrelated`: the ones read are right (an iOS share action for "share bookmarks", the tag section of the admin
    for "remove a bookmark"), except `api.md#Bookmarks` for "remove a bookmark": its `DELETE` endpoint is after
    the first 1,500 chars, which is all the judge sees.
  * The contradiction is real: `admin.md#Bookmark management` lists Delete, Archive and Unarchive as the admin's
    bulk actions, `bookmarks/admin.py` also has `mark_as_read` and `mark_as_unread`. The doc is out of date.
* The truncation is the main weakness of the judge: 16 of the 113 sections are over 1,500 chars, but they hold
  126 of the 196 matches (long sections share the most words), so most answers are given on a cut text.

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
2. [x] Match replay: recall of the current match, of `search_any` with a minimum BM25 score, and of the name alone,
   on the 20 labelled items, from the run's artifacts and docs, no LLM; verified by a recall / sections-per-item /
   judge-batches table in `# Findings`.
3. [x] Judge audit: read the verdicts on the matched items (false `covers`, real contradictions, unparsed answers);
   verified by the counts in `# Findings`.
4. [x] (only if a variant of 2 clearly wins) Amend REC-2, test, code; verified by `just check` and a replay of 2
   with the new match; makes true REC-2.

## Decisions
- Mode: autonomous
- Base commit: 46033390274ee801899852d8850323e28488242b
- 1a: the shown run uses hosted DeepSeek (`--provider deepseek`).
- 2a: the replay tool is a throwaway Rust example, not committed; only the figures go in the issue.
- 3a: the run goes through `just benchmark --series shown`, 1 run.
- After 3, 1a: REC-2 keeps the current match and, for an item it matches nothing, falls back on half of the
  content words of the name alone.
- After 3, 2a: the 1,500-char cut of the sections sent to the judge becomes its own issue.
- After 3, 3a: `retrodoc report` bailing on a `--no-confidence` run is only noted here (Deliverable 1).

## Deliverable 1

done: shown run on linkding (DeepSeek), status counts and 20 hand labels in `# Findings`.

| check | result |
|---|---|
| run | `generate` ok, 3 WARN schema refused (DeepSeek, expected), 1 WARN grounding (use cases) |
| tests, lint, specs, docs | n/a (no code) |
| code review | n/a (diff under `issues/` only) |

red: none. Not verified: `retrodoc report` bails ("no confidence scores found") on a `--no-confidence` run; the
reconciliation was read from `docs/_retrodoc/coverage-report.md` and `.retrodoc/cache/reconciliation.yaml`.

## Deliverable 2

done: replay of 8 match variants on 25 + 6 labelled items, table in `# Findings`.

| check | result |
|---|---|
| tests, lint, specs, docs | n/a (no committed code) |
| code review | n/a (diff under `issues/` only) |

red: none. Not verified: the labels are one reader's (mine); a section that only names a feature in a list
(README feature overview) counts as documenting it.

## Deliverable 3

done: audit of the 196 judge answers of the run, counts in `# Findings`.

| check | result |
|---|---|
| tests, lint, specs, docs | n/a (no code) |
| code review | n/a (diff under `issues/` only) |

red: none. Not verified: the 12 false `covers` come from reading every `covers` once, not a second reader;
the `unrelated` answers were read on a sample only.

## Deliverable 4

done: REC-2 amended and `candidates` falls back on the name alone when the name and description match no
section; replayed with the real `candidates()` on linkding: recall 18/25 (was 8/25), false 5/6, 285 items
matched, 604 sections, 36 judge batches (was 15).

| check | result |
|---|---|
| tests | `just check` ok |
| lint | clippy ok |
| specs | `just check-specs` ok (REC-2 traced) |
| docs | `docs/crates/pipeline.md` |
| code review | see below |

red: `an_item_its_description_matches_nowhere_is_searched_with_its_name_alone`: "the name alone finds it",
left `[]`, right `["docs/api.md#Tags"]`.
Not verified: the judge on the 166 items newly matched (no new `generate` run); its `unrelated` answers on
them rest on deliverable 3's audit.

Code review (low): no findings; its open question, a name of stop words only, is covered by `Bm25::search`
(a query of stop words finds nothing).

Follow-up (decision 2a): `issues/reconciliation_judge_sees_whole_sections.md`, queued second.

## Deliverable 5: fixes of the high review

High review of `4603339..HEAD`, 7 findings:

| finding | done |
|---|---|
| name searched only when the query matches nothing, a weak match blocks it | not fixed: decision 1a (fallback, not union), measured in deliverable 2 |
| half of a two-word name is one generic word | not fixed: the trade-off measured and taken in 1a; the judge answers `unrelated` |
| a rule is searched twice with its statement | fixed: no fallback when the name is the query |
| doc comments of `Item.name` / `Item.query` stale | fixed |
| fallback tested for features only | fixed: the test covers a domain, a feature and a use case |
| assertion message misleading | fixed |
| queue still lists this issue first | left to `just close-issue` |

| check | result |
|---|---|
| tests, lint, specs | `just check` ok |
| docs | n/a (doc comments only) |
| code review | low, see below |

red: none (the double search is not observable; the widened test passed at once, the code already did it).

Code review (low): no findings.
