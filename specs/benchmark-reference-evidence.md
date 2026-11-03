---
id: BRE
status: draft
adrs: []
---

# Evidence of the benchmark reference

## Intent

Every recall and precision of the quality benchmark rests on the hand-written reference, drafted by an agent. Each
feature of a reference cites the evidence that backs it, in the repository at its pinned commit or in its
maintainers' pages, so a script can check that the evidence exists and the figures can say how much of them rests on
human-written docs and how much moves with the features two independent drafts disagreed on.

## Requirements

- **BRE-1** (deterministic) A reference must load when its features carry no `evidence`, no `disputed` and the
  reference no `checked`, so the references written before them still score.
- **BRE-2** (deterministic) A feature must accept an `evidence` list whose items are each one of `doc: <path>` or
  `doc: <path>#<heading>` (the repository's human-written docs), `url: <url>` (a page of the maintainers) or
  `code: <path>` (a route, controller, job or test file).
- **BRE-3** (deterministic) An evidence item with no kind, two kinds or an empty value must fail the load with an
  error naming the feature.
- **BRE-4** (deterministic) The evidence check must report each `doc` or `code` item whose path does not exist in
  the given tree, naming the feature and the path.
- **BRE-5** (deterministic) The evidence check must report each `doc` item whose heading is not a heading of its
  file, a Markdown heading or an HTML `h1` to `h6` whose text equals it once case and surrounding spaces are ignored.
- **BRE-6** (deterministic) The evidence check must count the features by what backs them: a `doc` or `url` item,
  only `code` items, or no evidence.
- **BRE-7** (deterministic) The evidence check must exit with a failure status when it reports a missing path or
  heading.
- **BRE-8** (deterministic) Given saved run reports, the evidence check must print, per report, the strict and judged
  feature recall and precision with the features marked `disputed` taken out of the reference: a generated name
  counts as matched when one of its reference features is not disputed.
- **BRE-9** (deterministic) A reference must accept a `checked` entry with a `date` and a `method`, and the evidence
  check must print it, or say the reference has none.

## Scenarios

- (BRE-1) Given the reference of delivery_router as it is today, then it loads and scores as before.
- (BRE-3) Given an evidence item `{doc: a.md, code: b.py}`, then the load fails naming its feature.
- (BRE-5) Given `doc: docs/search.md#Search syntax` and a file holding `## Search Syntax`, then nothing is reported;
  given `<h2>Story merging</h2>` in an `.erb` file and the heading `Story merging`, then nothing is reported.
- (BRE-8) Given a report whose judged features match "Save a bookmark" (not disputed) and "Read it later" (disputed)
  from two generated names, and a reference of 10 features with 1 disputed, then the judged recall without the
  disputed features is 1/9 and the precision counts one generated name of two as matched.

## Non-goals

- Fetching the `url` items: they are recorded, a page of the maintainers can move or change after the pinned commit.
- Checking that the evidence says what the feature says: that is the work of the drafts and of the agent that
  settles their disagreements, recorded in `disputed`.
- Evidence for the domains, the use cases, the rules, `matches.yaml` and `entry-points.yaml`: a domain is backed by
  its features, the entry points are read from the route files `entry-points.yaml` lists, and the pairs of
  `matches.yaml` judge generated names, not the reference.
- Changing the figures `benchmark` and `benchmark-table` report: the figures without the disputed features are a
  measure printed by the check, not a column of the table.
