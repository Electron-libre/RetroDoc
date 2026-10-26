---
id: REC
status: accepted
adrs: [0019, 0033]
---

# Reconciliation of the generated docs with the existing ones

## Intent

Tell the reader of the debt report which domains, features, use cases and business rules the project's own
documents already cover, which they leave out and where they contradict what the code shows, each verdict
backed by the doc sections it rests on.

## Requirements

- **REC-1** (deterministic) The matched corpus must be the sections of the existing docs (one section per heading,
  the sections of one file under the same heading joined)
  and must not hold any file of the generated docs dirs (`functional/`, `_retrodoc/` under the docs dir).
- **REC-2** (deterministic) Each domain, feature, use case and business rule must be matched against the corpus with
  its name and description (a rule: its statement) as the query, keeping at most 3 sections, best first, that share
  at least half of the query's content words; when no section does, the query is the name alone, under the same
  rule; an item whose query has no content word is left out of the reconciliation (it can't be judged).
- **REC-3** (deterministic) An item with no matched section must be `undocumented` without an LLM call; a business
  rule that cites a doc section of the corpus in its evidence must be `documented` by that section without an LLM
  call; a cited section no longer in the corpus is ignored.
- **REC-4** (deterministic) The items with matched sections must be judged by the LLM in batches, each section
  answered `covers`, `contradicts` or `unrelated`; the item is `contradicted` when one section contradicts it,
  else `documented` when one covers it, else `undocumented`.
- **REC-5** (deterministic) A `contradicts` answer must carry what the section states and what the generated item
  states; an answer without both, a missing answer or one that can't be parsed counts as no answer for that section,
  and an item that no answer decides stays `matched` (documented, not judged) rather than get an invented verdict;
  an item with a section left unanswered is not saved, so the next run asks it again.
- **REC-6** (deterministic) A verdict must be reused with no LLM call while the fingerprint of the item's text, of
  its matched sections' text, of the model and of the rule that picks what the judge is shown of a section (REC-15)
  is unchanged; any change of one of them must judge the item again.
- **REC-7** (deterministic) The verdicts must be saved in `.retrodoc/cache/reconciliation.yaml`; `retrodoc report`
  and `render` must read them and make no LLM call. Without that file, or for an item whose text or sections changed
  since its verdict, a matched item is `matched`.
- **REC-8** (deterministic) The pass must make no LLM call when the corpus is empty or no item has a matched
  section, and must not run when `[pipeline] reconcile` is `false`.
- **REC-9** (deterministic) The report must give, per domain, how many of its items are documented, matched,
  undocumented and contradicted.
- **REC-10** (deterministic) The report must list the undocumented items ranked by the number of entry points they
  hold (a domain: those of its features; a rule: those of its feature), then by confidence (highest first, unscored
  last), then by label; at most 50 are listed, the rest counted.
- **REC-11** (deterministic) The report must list each contradiction with the item, the doc section (`path#heading`),
  what the section states and what the generated item states; it presents them as findings to check, not as
  errors of the doc.
- **REC-12** (deterministic) The page of a domain, feature or use case, and the line of a rule, must link each section
  that documents or contradicts it, with a relative link to the doc file that resolves from the page; an
  undocumented item gets no such section. The render stays deterministic.
- **REC-13** (deterministic) The former "Business rules without documentation" section of the report must be
  replaced by the lists of REC-10 and REC-11, the rules being items like the others.
- **REC-14** (deterministic) An LLM call of the pass that fails must not fail `generate`: the pass stops with a
  warning, keeping the verdicts got before.
- **REC-15** (deterministic) A request of the judge must show at most 36,000 chars of section text. A section within
  an even share of what is left is shown whole and gives the rest back to the others, which share it evenly, never
  less than 1,500 chars each. A longer section must be shown as its heading line plus the paragraphs (split on
  blank lines, a fenced code block being one paragraph) that BM25 ranks against the query of each of the batch's
  items it is matched to, taken in turn for each of those items, best first, until its share is spent, then shown
  in the order of the section with `[...]` where paragraphs are left out. A paragraph longer than the share divided
  by the number of those items is cut to it, a code fence it opens closed; a section where no paragraph holds a
  query word is shown from its start, cut to its share.

## Scenarios

- (REC-1) Given a `docs/functional/billing/README.md` generated by a previous run that names every feature, when the
  report is built, then no item is matched to it.
- (REC-4) Given a use case matched to two sections, one answered `covers` and one `contradicts`, then the use case is
  `contradicted` and both sections are kept with their answers.
- (REC-4) Given a feature whose only matched section is answered `unrelated`, then it is `undocumented` and listed
  as such.
- (REC-6) Given a second run where only one doc section changed, then only the items matched to that section, before
  or after the change, are sent to the LLM.
- (REC-12) Given a use case page under `docs/functional/billing/pay-invoice.md` documented by
  `docs/guide.md#Paying`, then the page links `../../guide.md#paying`.

- (REC-15) Given a 4,700-char section whose `Delete` paragraph is past its first 1,500 chars, matched to the use
  case "remove a bookmark" in a batch of 8 sections, then the `Delete` paragraph is shown to the judge.

## Non-goals

- Correcting the existing docs or the generated ones: a contradiction may come from an out-of-date doc and is only
  shown.
- Semantic (embedding) retrieval: the matching is lexical, a paraphrase with no shared word is missed.
- Whether the LLM's `covers` / `contradicts` answer is right: that is a quality measure, not a requirement.
- Reconciling with external sources (wikis, tickets) or with the technical docs.
