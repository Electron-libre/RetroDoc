---
rank: 1000
why: 'open point of `docs/findings.md` (2026-10-30): small, milestone `Ask the documentation` comes last; before the MCP edge cases, same crate'
---
# Return nothing from the documentation search when nothing is relevant

# Goal
`retrodoc search` and the `search_docs` MCP tool rank the generated and collected docs with BM25 and return the top
hits whatever their score. A question on a topic the docs do not cover still returns entries that share one common
word, so an agent cannot tell "not documented" from a weak match, while the product asks it to say "not documented"
rather than guess. Expected: hits below a relevance floor are left out, and an empty answer says the topic is not
documented.

# Findings (smoke repository `delivery_router`, `docs/findings.md` §4 step 1)
* Right use case found for a business question, but an undocumented topic still returned entries sharing a common
  word; the stop-word filter and the half-of-the-words rule do not fix it with two-word queries.
* `crates/retrodoc-mcp/src/search.rs:56`: `search` takes the BM25 hits in order up to `limit`, with no score floor.
* No synonym matching: business words that differ from the docs' find nothing (to measure before adding anything).

# Approach
1. Measure first: a small hand-written set of questions per smoke and benchmark repository, documented and not, with
   the scores of their hits, to see whether a floor separates them (absolute, or relative to the best hit).
2. Candidates: (a) a relative floor (a share of the best score); (b) an absolute floor normalized by the query length;
   (c) the answer names the floor so the agent can widen its query.
3. Synonyms stay out unless the measure shows business vocabulary missed (`PRODUCT.md`, open questions).

# Milestone
Ask the documentation

# Resources
* `crates/retrodoc-mcp/src/search.rs`, `crates/retrodoc-mcp/src/tools/`, `crates/retrodoc-pipeline/src/bm25.rs`.
* ADR 0016 (read-only, LLM-free MCP server).
* `docs/findings.md` §4.

# Hints
* `bm25.rs` is shared with the reconciliation and the brief evidence: a floor in the search must not change them.

# Tracking

Plan: validated

## Understanding
Leave out of `retrodoc search` / `search_docs` the hits that only share the query's common words, and say "not
documented" with the number left out. Out of scope: synonyms, the BM25 of the reconciliation and the brief evidence.
Means: neither candidate (a) nor (b) of the issue; a floor on the share of the query's IDF a hit holds (measure below).

## Measure (no LLM, 2026-10-31, `~/retrodoc-runs/search_relevance/`)
42 hand-written questions (21 documented, 21 not) on the saved runs of delivery_router (`rsa-dr`), linkding
(`rules-ld-dedup`) and healthchecks (`rsa-hc`); `scores.txt` (scores), `coverage.txt` (words matched by the top hit).
* Absolute score: no floor separates them (documented top hits from 0.71, undocumented up to 14.1).
* Relative floor (a): only trims the tail, it can't empty an answer whose best hit is itself weak.
* Share of the query's IDF held by the top hit: documented 1.00 (15), 0.52-0.68 (3), 0.06-0.22 (3: `nearest`,
  `address`, `rename` absent, synonym misses); undocumented 0.06-0.50 (10), 0.53 (1), 1.00 (2: `cancel an order`,
  `delete a user account`, whose words the docs do have). A floor "more than half" keeps 18/21 documented and drops
  11/14 undocumented of those with a hit (7 already returned nothing).

## Plan
1. [x] IDF-share floor in the documentation search, with the not-documented answer naming the hits left out:
   `Bm25` gets a ranking with the floor (the `search`/`search_any` of the reconciliation and the brief untouched),
   `SearchIndex::search` uses it, `search_docs` and `retrodoc search` say not documented with the count; verified by
   unit tests in `bm25.rs`/`search.rs` and an end-to-end test of `retrodoc search` (`crates/retrodoc-cli/tests/search.rs`);
   makes true DSR-1..DSR-4 (spec `accepted`). ~250 lines.

## Decisions
- Mode: autonomous
- Base commit: e1bcab4f0da6cc40299361b59f5f9191f7dd5f6d
- Floor: strictly more than half of the query's IDF weight (measured best point).
- The half-of-the-words rule stays; the floor comes on top of it.

## Deliverable 1 summary
done: `Bm25::search_relevant` (IDF-share floor, half-of-the-words rule kept), used by `SearchIndex::search`; the
not-documented answers of `search_docs` and `retrodoc search` give the number of entries left out; spec `accepted`.
Re-measured on the 42 questions (`after.txt`): 19/21 documented answered, 17/21 undocumented answer "not documented".

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | DSR-1..4 traced, `accepted` |
| docs | `docs/crates/render-mcp-cli.md`, `docs/findings.md` §4 |
| code review | see below |

red: DSR-2, DSR-3, DSR-4: `a_query_whose_rare_word_no_entry_has_is_not_documented` listed `["Assign a rider", "Track a rider", "Plan a route"]` where none was expected.
Code review: no findings

not verified: nothing.
decisions needed: none.
2. [x] Docs of the whole issue and fixes of its `high` review: the not-documented answer names the query words no
   entry has (DSR-5: a typo or a filler word is the heaviest, the agent sees it), a margin on the half-weight
   floor against rounding, "1 entry shares" / "N entries share only part of the query", `retrodoc search --limit 0`
   refused, the projections of `bm25.rs` shared, the MCP diagram of `docs/ARCHITECTURE.md`.
   Not fixed: `Found` kept beside `Relevant` (it holds entries, not indexes); "Try other words" stays in every MCP
   not-documented answer (it was there before; DSR-4 asks it when entries were left out, not only then); filler
   words not added to the stop words (shared with the reconciliation, which must not change).

## Deliverable 2 summary
done: fixes of the `high` review above.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | DSR-5 added and traced, `accepted` |
| docs | `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md` |
| code review | see below |

red: DSR-5: none recorded before the code (the test was written with the fix, after the `high` review).
Code review: no findings

not verified: nothing.
decisions needed: none.
