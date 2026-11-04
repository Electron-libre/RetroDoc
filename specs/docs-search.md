---
id: DSR
status: draft
adrs: [0016]
---

# Documentation search

## Intent

`retrodoc search` and the `search_docs` tool of the MCP server find, among the generated entries (domains, features,
use cases, rules, glossary) and the collected docs, those that answer a question, so an agent can tell a documented
topic from an undocumented one and say "not documented" rather than guess.

## Requirements

- **DSR-1** (deterministic) The search must return the matching entries best first, at most as many as the limit asked.
- **DSR-2** (deterministic) The search must not return an entry whose words in common with the query weigh half of the
  query's weight or less, each word of the query (stop words left out) weighing by how rare it is among the entries, a
  word no entry has weighing the most.
- **DSR-3** (deterministic) When no entry is returned, the answer of `search_docs` and the output of `retrodoc search` must
  say that the topic is not documented.
- **DSR-4** (deterministic) The not-documented answer must give the number of entries that share words with the query
  but that DSR-2 left out, and ask for other words, when there are any.
- **DSR-5** (deterministic) The not-documented answer must name the words of the query, as written, that no entry has.
- **DSR-6** (deterministic) The search must find an edited section by the words of its edit, and not by the words
  of the generated field the edit replaces (`specs/section-edit.md`).

## Scenarios

- (DSR-2) Given entries that all mention `rider` and none `salary`, when the query is `rider salary`, then nothing is
  returned.
- (DSR-2) Given a query of one word that some entries have, then those entries are returned: the word is the whole
  weight.
- (DSR-2) Given a query of two words as rare as each other and an entry that has only one of them, then the entry is
  not returned: it holds half of the weight, not more.
- (DSR-2) Given a query whose rare word an entry has and whose common word it lacks, then the entry is returned.
- (DSR-5) Given `cancle subscription` where `cancle` is a typo, then the answer names `cancle`, so the agent can
  fix its query rather than conclude the topic is undocumented.
- (DSR-4) Given no entry sharing any word with the query, then the answer says not documented without a number of
  partial matches.

## Non-goals

- No synonym or paraphrase matching: a question worded differently from the docs finds nothing.
- The ranking used by the reconciliation of the existing docs and by the evidence of the product brief keeps no such
  floor: only the documentation search applies it.
- No floor on the score itself: scores are not comparable from one corpus or one query to another.
