---
id: MCP
status: draft
adrs: [0044]
---

# MCP server answers

## Intent

The read-only MCP server lets an agent browse the generated functional docs by id (`get_domain`, `get_feature`,
`get_use_case`): each id resolves to one documented item or to a "Not documented" answer that points to the way out,
so the agent never reads an item it did not ask for. Each of these answers also says what a human said of the
item (`specs/section-review.md`), so the agent knows what was checked or rejected beside the model's confidence.

## Requirements

- **MCP-1** (deterministic) `get_domain` must answer "Not documented" to any id that is not the slug of a domain.
- **MCP-2** (deterministic) `get_feature` must answer the feature to `<domain>/<feature>` and to the feature slug alone
  (feature slugs are unique across domains), and "Not documented" to an id whose domain segment is not the domain of
  that feature.
- **MCP-3** (deterministic) `get_use_case` must answer "Not documented" to `<domain>/<feature>/<use case>` or
  `<feature>/<use case>` when a segment given is not the feature or the domain of that use case.
- **MCP-4** (deterministic) `get_use_case` must answer a use case slug alone with the one use case that has it, or,
  when several have it, with "Not documented" and the full id of each candidate.
- **MCP-5** (deterministic) The "Not documented" answer to an unknown id must name the id, the tool to browse with
  and, for a domain, the known domains, leaving their clause out when there are none.
- **MCP-6** (deterministic) A list item must show the name on one line and, as snippet, the first non-blank line of
  the description.
- **MCP-7** (deterministic) `get_domain`, `get_feature` and `get_use_case` must show, on the line after the
  confidence, the review of the section answered (REV-1): its status, its comment when it has one, its author when
  it has one and its date; with no review, that it is not reviewed.
- **MCP-8** (deterministic) The answer of a section marked `wrong` must open, before its title, with a warning that a
  human marked it wrong, quoting the comment when it has one.
- **MCP-9** (deterministic) The reviews must be read from `.retrodoc/reviews.json` at each answer, so that a review
  saved while the server runs (from `retrodoc ui`) shows in the next answer without a restart.
- **MCP-10** (deterministic) An answer must show, in place of the review line, the error of a
  `.retrodoc/reviews.json` that cannot be read or is of a later format (REV-10), and the rest of the answer as
  without it.
- **MCP-11** (deterministic) A comment over several lines must show on one line in the review line and the warning,
  so that it forges no line of the answer.
- **MCP-12** (deterministic) The review line of an outdated review (REV-20, REV-21) must say it is outdated, with its
  reasons: the section rewritten since the review, the files changed since the review.
- **MCP-13** (deterministic) The warning of a section marked `wrong` (MCP-8) whose review is outdated must say the
  section changed since it was marked wrong.
- **MCP-14** (deterministic) `list_domains`, `get_domain`, `get_feature` and `get_use_case` must show the name, the
  description and the narrative of an edited section as its edit gives them (`specs/section-edit.md`).

## Scenarios

- (MCP-1) Given the domain `billing`, when `get_domain` is asked `billing/payment`, then it answers "Not
  documented".
- (MCP-2) Given `pay-invoice` in `billing`, when `get_feature` is asked `shipping/pay-invoice`, then it answers "Not
  documented"; asked `pay-invoice` or `billing/pay-invoice`, it answers the feature.
- (MCP-3) Given `pay-by-card` of `pay-invoice` in `billing`, when `get_use_case` is asked
  `shipping/pay-invoice/pay-by-card` or `cancel-plan/pay-by-card`, then it answers "Not documented".
- (MCP-4) Given `pay-by-card` in both `pay-invoice` and `pay-order`, when `get_use_case` is asked `pay-by-card`, then
  it answers "Not documented" and lists `billing/pay-invoice/pay-by-card` and `billing/pay-order/pay-by-card`.
- (MCP-5) Given no domain generated, when `get_domain` is asked `billing`, then the answer has no "Known domains"
  clause.
- (MCP-6) Given a description starting with a blank line, then `list_domains` shows its first written line.
- (MCP-7) Given a feature validated with no comment and no author (no `user.name`), then the review line shows its
  status and date only.
- (MCP-7) Given a use case `pay-by-card` of `pay-invoice` reviewed, when `get_use_case` is asked
  `pay-by-card`, then its review shows: the slug alone names the same section as `pay-invoice/pay-by-card`.
- (MCP-8) Given a domain marked `to_review`, then its answer has no warning.
- (MCP-9) Given the server started before any review, when a feature is marked wrong, then the next `get_feature`
  of it opens with the warning.

## Non-goals

- No review in `list_domains` nor in the items listed inside an answer (the features of a domain, the use cases of
  a feature): the agent sees the review when it reads the item.
- No review of a business rule (REV non-goal).
- No fuzzy matching of ids (typos, case): `search_docs` finds what an id misses.
- No resolution of two features sharing a slug across domains: the pipeline never produces them (slugs are made
  unique against every feature already produced), and a corpus built by hand with them reaches the first one only.
