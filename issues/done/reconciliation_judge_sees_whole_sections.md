# Let the reconciliation judge read the part of a long section that matters

# Goal

The judge of the reconciliation (`reconcile/judge.rs`) is shown the first 1,500 chars of each matched doc
section (`MAX_SECTION_CHARS`). Long sections are the ones BM25 matches most, so most answers are given on a
cut text: a section that documents the item after its first 1,500 chars is answered `unrelated`, and the first
part alone can be answered `covers` on a field name. The judge should see the part of the section the item is
about, with the request still bounded.

# Findings

* 2026-10-26, linkding, `deepseek-chat`, measured in `issues/done/reconciliation_recall_on_real_docs.md`
  (deliverable 3): 16 of the 113 doc sections are over 1,500 chars, and they hold 126 of the 196 matches.
* `api.md#Bookmarks` (4,700 chars) lists the bookmark endpoints with bold labels, not headings: its `DELETE`
  endpoint is past the cut, and "remove a bookmark" was answered `unrelated` on it.
* False `covers` on the same section for tags, bundles and favicons: its first part shows a `tag_names`
  field, a `bundle` filter and a `favicon_url`.
* Since REC-2 falls back on the name, 285 items are matched instead of 119, so more answers rest on these
  long sections.

* 2026-10-26, measure of deliverable 1: copy of the saved linkding run in
  `~/retrodoc-runs/reconcile-sections/before`, reconciliation re-run with the REC-2 fallback (`deepseek-chat`,
  21 calls, 35 s). 604 answers, **242 on a section over 1,500 chars** (52 `covers`, 190 `unrelated`), 89 of them
  on `api.md#Bookmarks`, whose cut falls inside the JSON example of its first endpoint (`List`): the judge never
  sees `Create`, `Update`, `Archive`, `Unarchive` or `Delete`.
* Hand labels, every 6th of the 242 answers (41), in `~/retrodoc-runs/reconcile-sections/labels.yaml`: 30 right,
  9 wrong, 2 unsure. **6 of the 9 wrong are `unrelated` on `api.md#Bookmarks` for an item its endpoints past the
  cut document** (update, remove, unarchive a bookmark, update via the API); 1 more is right by luck (`covers` for
  "mark unread" on the `unread` field of the example). The 3 other wrong are false `covers` on the first part
  (tags on `api.md#Bookmarks`, two snapshot items on the Singlefile section): seeing more text may not fix them.
  On the other long sections the tail held nothing the answer needed.
* 2026-10-26, comparison of deliverable 3 on the 39 labelled answers (2 unsure left out), `deepseek-chat`, the
  whole reconciliation re-run each time (36 calls, about 1 min):

  | prompt | right | wrong, text past the old cut | wrong, other | prompt tokens |
  |---|---|---|---|---|
  | first 1,500 chars, run 1 / run 2 | 30 / 32 | 6 / 7 | 3 / 0 | 136k |
  | REC-15, paragraphs with the most query words | 36 | 1 | 2 | 158k |
  | REC-15, words weighted by rarity in the section, run 1 / run 2 | 35 / 36 | 1 / 1 | 3 / 2 | 158k |

  The wrong answer left past the cut is "Restore an archived bookmark" against the `Unarchive` endpoint: no word
  shared, a paraphrase the lexical pick can't find (non-goal). The other wrong ones move between runs (DeepSeek
  is not steady at temperature 0 on them): false `covers` of the metadata or bundles items by
  `api.md#Bookmarks`, of a snapshot item by the Singlefile section.
* The prompt of the batch of "Unarchive a bookmark" (11 sections, a share of 3,272 chars) showed the JSON example
  and the first endpoints instead of `Unarchive`: counting the query words a paragraph holds favours the long
  paragraphs full of `bookmark`, `list`, `archived`. Weighting each word by how few paragraphs of the section hold
  it shows `Unarchive` and `Delete` (REC-15 amended, `SHOWN_RULE` v2).
* REC-15 after the high review (paragraphs ranked by `Bm25`, unused shares given back, fences kept whole),
  `deepseek-chat`, one run: 36 of 39 right, **all 7 answers whose text is past the old cut right** (the
  "Restore an archived bookmark" one too), 3 wrong on the first part of a section (metadata by
  `api.md#Bookmarks`, a snapshot item by the Singlefile section, and the import item on `index.mdx#Features`,
  a short section shown whole as before: run noise); 169k prompt tokens for the 36 calls (136k before).
# Approach

1. Measure first: on the saved linkding run, list the answers given on a section longer than the cut and
   label by hand whether the text the item needs was past it.
2. Candidates (can be combined):
   1. Send the window of the section that holds the most query words of the batch's items, instead of its start.
   2. Split a long section on its bold labels or paragraphs into sub-sections before the match, so each part
      is matched and judged on its own (changes REC-1 and the `path#heading` origin).
   3. Raise the cut for a batch with few sections, keeping the total of a request under a fixed budget.
3. Compare the answers on the labelled sample before and after, with the hosted and the local model.

# Specs

* `specs/docs-reconciliation.md`: REC-4 (what the judge is shown), REC-1 if sections are split.

# Resources

* `crates/retrodoc-pipeline/src/reconcile/judge.rs` (`prompt`, `MAX_SECTION_CHARS`), `reconcile/mod.rs`
* `issues/done/reconciliation_recall_on_real_docs.md` (Findings, labels and replay tool kept with the saved run)
* ADR 0033

# Hints

* A section is listed once per batch and shared by several items: a window chosen per item breaks that.
* The verdict fingerprint holds the section's text: a change of what is sent should change it, or the saved
  verdicts are reused on a text the judge never saw.

# Tracking

Plan: validated

Status: done (2026-10-26). Left: the local comparison, in `issues/hybrid_llama_server_as_local_llm.md`.

## Understanding
The judge sees the first 1,500 chars of a section; show it instead the paragraphs of a long section that hold the
words of the batch's items, within a request budget no larger than today's worst case (24 x 1,500 chars).
Out of scope: splitting sections (REC-1 and the `path#heading` links stay), the match (REC-2). Means: candidates 1
and 3 of the issue combined (REC-15), the section still listed once per batch.

## Plan
1. [x] Measure: copy the saved linkding run, re-run the reconciliation with the REC-2 fallback (DeepSeek), list the
   answers on sections over 1,500 chars and label by hand, on a sample of 40, whether the text the item needs is
   past the cut and whether the answer is right; verified by the counts in `# Findings`.
2. [x] Excerpts: `judge::prompt` shows each section within its share of the 36,000-char budget, a long one by the
   paragraphs that hold the most query words of its items; the fingerprint holds a version of that rule; verified
   by unit tests (whole section, excerpts per item in turn, `[...]`, budget, fingerprint change); makes true REC-15,
   REC-6.
3. [x] Compare: re-run the reconciliation of 1 with the new prompt, DeepSeek and the local model, and score the
   labelled sample before / after; verified by a table in `# Findings`.

4. [x] Fixes of the high review: the paragraph pick ranked by `Bm25`, a long paragraph cut to the share of one item,
   no heading line for the text before the first heading, fences and blank lines with spaces, unused shares given
   back, the cap kept; verified by unit tests; makes true REC-15.

## Decisions
- Mode: autonomous
- Base commit: 6f3b2205981f4444461077f7f1ebd0f677693216
- 1a: paragraph excerpts within a per-request budget (REC-15), sections not split.
- 2a: 36,000 chars of section text per request.
- After 2: the local model comparison is left until the switch to the hybrid llama-server
  (`issues/hybrid_llama_server_as_local_llm.md`); deliverable 3 measures with DeepSeek only.
- High review: what is shown of a section depends on the other items of the batch, and the fingerprint does not
  hold it (an unchanged item keeps a verdict given on another excerpt). Kept: a fingerprint per batch would ask
  every item again when one changes (the hint of the issue); the excerpt changes only with the batch.
- 3a: the reconciliation is re-run by an incremental `generate` on a copy of the saved linkding run, DeepSeek.

## Deliverable 1

done: reconciliation re-run on a copy of the saved linkding run, 41 answers on long sections labelled, counts in
`# Findings`.

| check | result |
|---|---|
| run | `generate --no-confidence` ok, only `reconcile` called (21 calls) |
| tests, lint, specs, docs | n/a (no code) |
| code review | n/a (diff under `issues/` only) |

red: none. Not verified: the labels are one reader's; the replay tool (`long`, `text` added to
`~/retrodoc-runs/reconcile-replay`) is not committed (decision 2a of the previous issue).

## Deliverable 2

done: `judge::prompt` shares 36,000 chars between the sections of a request (`shown`), a long section by its
heading and the paragraphs holding its items' query words, each item in turn, `[...]` for the gaps; the
fingerprint holds `SHOWN_RULE`; REC-15 added, REC-6 amended, spec `accepted`.

| check | result |
|---|---|
| tests, lint, specs | `just check` ok |
| docs | `docs/crates/pipeline.md` (`reconcile/` row) |
| code review | see below |

red: `a_long_section_shows_the_paragraphs_of_each_of_its_items_in_turn`: "the archive paragraph is shown"
(the 1,500-char cut); the 4 others failed on the cut or on a reused verdict (`left: 0, right: 1` calls).
Not verified: the answers of the judge on the new prompt (deliverable 3).

Code review (low): no findings.

## Deliverable 3

done: before / after on the labelled sample with DeepSeek (table in `# Findings`); the paragraph pick weights
each word by its rarity in the section (REC-15 amended, `SHOWN_RULE` v2), after the prompt of a missed item showed
the JSON example instead of its endpoint.

| check | result |
|---|---|
| tests, lint, specs | `just check` ok |
| docs | `docs/crates/pipeline.md` (`reconcile/` row) |
| code review | see below |

red: `a_paragraph_with_a_word_rare_in_the_section_beats_ones_with_its_common_words`: the section shown ended on
the common paragraphs, no "Unarchives a bookmark.".
Not verified: the local model (left, decision after 2); one or two runs per variant, DeepSeek moves 2 answers of
39 between runs.

Left: the local comparison once the hybrid llama-server is the local target, moved to
`issues/hybrid_llama_server_as_local_llm.md`.

Code review (low): no findings.

## Deliverable 4: fixes of the high review

High review of `6f3b220..HEAD`, 10 findings:

| finding | done |
|---|---|
| the first paragraph picked, cut, takes the whole share | fixed: a paragraph is cut to the share of one item |
| a long hit paragraph skipped once something is kept | fixed: same cut, it always fits |
| the text before the first heading treated as a heading | fixed: no heading line when the section has none |
| a fence with blank lines split, left open | fixed: a fenced block is one paragraph, a cut fence closed |
| blank lines holding spaces don't split | fixed |
| the share of short sections lost | fixed: given back to the long ones (REC-15 amended) |
| what is shown depends on the batch, not in the fingerprint | not fixed: decision in `## Decisions` |
| the weight rescans every paragraph | fixed: `Bm25` over the paragraphs replaces it |
| a hand-made IDF next to `Bm25` | fixed: `Bm25::search_any` ranks the paragraphs |
| the last `[...]` not budgeted, cap overrun | fixed: set aside, the tests check `<= 36,000` |

| check | result |
|---|---|
| tests, lint, specs | `just check` ok |
| docs | `docs/crates/pipeline.md` (`reconcile/` row) |
| code review | see below |

red: the 5 new tests, e.g. `a_short_section_gives_its_unused_share_to_the_long_ones`: "3926 chars: the share the
short sections left"; `a_long_paragraph_first_picked_leaves_room_for_the_other_items`: no "Deletes a bookmark.".
Not verified: one DeepSeek run only for this version (table in `# Findings`); the local model (decision after 2).

Code review (low): no findings.
