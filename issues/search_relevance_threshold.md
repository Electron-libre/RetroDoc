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
