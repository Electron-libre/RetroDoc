---
id: DBT
status: accepted
adrs: [0039, 0044]
---

# Debt view of the docs

## Intent

The debt page of `retrodoc ui`, `/debt`, lists as one list the sections of the docs of the last run that need a
human, the code no doc covers and the items the project's own docs contradict or leave out, so whoever audits the
docs sees what to check and filters it by kind and domain, instead of reading the debt report of `retrodoc report`
from top to bottom.

## Requirements

- **DBT-1** (deterministic) The debt page, `/debt`, must list the debt items of the last run, each with its kind,
  the name of its domain, its section (the use case with its feature, or the feature), its score or its reason, and a
  link to the page of its section.
- **DBT-2** (deterministic) An item of the kind "low confidence" must be listed for each use case whose confidence is
  below `LOW_CONFIDENCE_THRESHOLD`, and each feature below it that has no use case, with its score and its
  rationale: the sections the low-confidence section of `retrodoc report` lists.
- **DBT-3** (deterministic) An item of the kind "not scored" must be listed, when at least one use case of the run
  has a confidence, for each use case with none; when none has one, the page says the run has no confidence
  score and lists no item of that kind.
- **DBT-4** (deterministic) An item of the kind "unbacked narrative" must be listed for each use case whose narrative
  the confidence pass judged unsupported by the cited code (`narrative_check` at 0), with its rationale.
- **DBT-5** (deterministic) An item of the kind "technical language" must be listed for each use case whose
  business-language score is below `LOW_CONFIDENCE_THRESHOLD`, with its score and its rationale.
- **DBT-6** (deterministic) An item of the kind "set apart" must be listed for each use case the docs set apart
  under "Administration and technical" (`UseCase::is_technical`), with its reason: its back-office feature, its
  business language or its confidence.
- **DBT-7** (deterministic) An item of the kind "stale" must be listed for each feature and each use case flagged
  stale on its page (WEB-20), with the files that changed.
- **DBT-8** (deterministic) The items must be ordered by their priority (DBT-23), the highest first, and items of
  equal priority by kind in the order of DBT-2 to DBT-7, DBT-15, DBT-16, DBT-30, DBT-31, DBT-19 and DBT-20, then
  within a kind as `retrodoc report` lists them (the lowest score first, then by section; the unbacked narratives by
  section; the undocumented items most entry points first), the not scored, set apart, stale and contradicted ones in
  the order of the run, a stale feature before its use cases, the reviewed sections as the reviews are saved (REV-15),
  the folders of DBT-21 by path.
- **DBT-9** (deterministic) The debt page must keep only the items of the kind `?kind=<kind>` and of the domain
  `?domain=<slug>`, each given, both when both are; an empty value filters nothing.
- **DBT-10** (deterministic) The debt page must show a form whose choices are the kinds and the domains of the run,
  the ones of the query selected, that asks the page again with them.
- **DBT-11** (deterministic) The debt page must say no section needs a human when the run has no item and is scored
  (DBT-3 says the rest), and that no item matches when the filters keep none.
- **DBT-12** (deterministic) A debt request must be answered, with the header `HX-Request` and without
  `HX-History-Restore-Request`, with the list of the items alone, without the layout of the page; any other with
  the full page.
- **DBT-13** (deterministic) Every page must link to the debt page under an "Audit" group of its header, and the
  "Start here" section of the home page must link to it with its number of items.
- **DBT-14** (deterministic) On a repository with no run yet, the debt page must say to run `retrodoc generate`.
- **DBT-15** (deterministic) An item of the kind "contradicted" must be listed for each domain, feature, use case
  and business rule that the reconciliation of the run with the project's docs (`specs/docs-reconciliation.md`, from
  the saved verdicts, no LLM call) finds contradicted, with, for each contradicting section, what it states, what the
  docs state and a link to that section on the page of its collected doc.
- **DBT-16** (deterministic) An item of the kind "undocumented" must be listed for each domain, feature, use case and
  business rule that the reconciliation finds mentioned in no section of the project's docs, with its number of
  entry points.
- **DBT-17** (deterministic) When the project has no collected doc, the page must say there is no doc to compare
  the items with, and list no item of the kinds of DBT-15 and DBT-16.
- **DBT-18** (deterministic) The items of DBT-15 and DBT-16 must name the kind of their section (domain, feature, use
  case, rule), carry its domain and link to its page.
- **DBT-19** (deterministic) An item of the kind "entry point without a feature" must be listed for the entry points
  of the saved behaviour coverage that no feature holds.
- **DBT-20** (deterministic) An item of the kind "no domain" must be listed for the source files the clustering left
  in no domain, of the kind "reached by no behaviour" for the source files of the saved behaviour coverage that no
  behaviour reaches, and of the kind "not analysed" for the source files the file budget left out (`scope.yaml`).
- **DBT-21** (deterministic) The items of DBT-19 and DBT-20 must be one item per kind and folder (the directory of
  the file, `.` for the root of the repository), with the number of its files or entry points and their list, each
  file a link to its page `/code/{path}` and each entry point to the page of its file.
- **DBT-22** (deterministic) The items of DBT-19 and DBT-20 must carry no domain: a `?domain=` filter keeps none of
  them.

- **DBT-23** (deterministic) The priority of an item must be `weight × weakness × (1 + ln(1 + commits)) × (1 + ln(1 +
  entry points))`, from the four parts of DBT-24 to DBT-26 and DBT-29.
- **DBT-24** (deterministic) The weakness of an item must be `1 − score` for an item with a score (DBT-2, DBT-5) and 1
  for any other.
- **DBT-25** (deterministic) The commits of an item must be the most commits of one of its files, the commits of a
  file being those of the history of `HEAD` read when `retrodoc ui` starts that changed it; the files of an item are
  the `source_paths` of a feature, the files the steps of a use case cite, the files of the features of a domain, the
  files of the code evidence of a rule, the files of the entries of a folder.
- **DBT-26** (deterministic) The entry points of an item must be those of its feature or use case, the number of
  entry points of a doc item (DBT-16), the number of entry points of a folder of DBT-19, and 0 for a folder of DBT-20.
- **DBT-27** (deterministic) Each item must show its priority and its weight, weakness, commits and entry points.
- **DBT-28** (deterministic) When the repository has no readable git history, the page must say the commits are
  unknown, and every item counts 0 commits.
- **DBT-29** (deterministic) The weight of an item must be 1 for the kinds whose section the run or a human suspects
  false (low confidence, unbacked narrative, stale, contradicted, marked wrong), 0.5 for the kinds of a section
  missing, unreadable or left to check (not scored, technical language, undocumented, entry point without a feature,
  to review), and 0.25 for the kinds the docs leave out by design (set apart, no domain, reached by no behaviour, not
  analysed).
- **DBT-30** (deterministic) An item of the kind "marked wrong" must be listed for each domain, feature and use case
  of the run whose review (`specs/section-review.md`) has the status `wrong`, with the comment of the review as its
  reason.
- **DBT-31** (deterministic) An item of the kind "to review" must be listed for each domain, feature and use case of
  the run whose review has the status `to_review`, with the comment of the review as its reason.
- **DBT-32** (deterministic) The kinds low confidence, not scored, unbacked narrative and technical language must
  leave out each feature and use case whose review has the status `validated` and is not outdated (REV-20, REV-21).
- **DBT-33** (deterministic) An item of a domain (DBT-30, DBT-31) must link to the page of the domain, its files being
  the `source_paths` of the features of the domain and its entry points the sum of theirs (DBT-25, DBT-26).
- **DBT-34** (deterministic) The debt page must read the reviews at each request, and when the file of the reviews
  cannot be read, say so and list the debt as if no section was reviewed.
- **DBT-35** (deterministic) The debt report of `retrodoc report` must list, under a heading "Sections marked wrong"
  shown only when one is, each domain, feature and use case of the run marked wrong, with its kind, its label and the
  comment of its review, in the order the reviews are saved (REV-15).
- **DBT-36** (deterministic) When the file of the reviews cannot be read, `retrodoc report` must warn of it and print
  the report without the reviews.
- **DBT-37** (deterministic) An item of the kinds "marked wrong" and "to review" whose review is outdated must say so,
  with its reasons: the section rewritten, the files changed since the review.
- **DBT-38** (deterministic) A section of "Sections marked wrong" of `retrodoc report` whose review is outdated must
  say so, with its reasons as DBT-37 says.

## Scenarios

- (DBT-1) Given a use case whose feature is not in the run (artifacts of an older run), then it is listed without a
  link and no `?domain=` keeps it.
- (DBT-2) Given a feature below the threshold that has use cases, then it is not listed: its use cases explain it.
- (DBT-4) Given a narrative judged partly backed (`narrative_check` above 0), then it is not listed.
- (DBT-6) Given a use case set apart for two reasons, then it is one item naming both.
- (DBT-7) Given a cited file with no recorded hash (left out by the file budget), then no section citing it is stale.
- (DBT-2, DBT-5) Given a use case both low in confidence and in business language, then it is listed under both kinds.
- (DBT-9) Given a filter whose value names no kind or domain of the run, then no item matches.
- (DBT-10, DBT-1) Given a section name `<script>`, then it is shown as text (WEB-10).
- (DBT-15, DBT-16) Given an item matched by sections no saved verdict judged (`matched`), then it is not listed.
- (DBT-15) Given an item that two sections contradict, then it is one item naming both.
- (DBT-16) Given a rule whose feature is no longer in the run, then it is listed without a link.
- (DBT-21) Given the files `app/a.rb`, `app/b.rb` and `lib/c.rb` reached by no behaviour, then two items are listed,
  `app` with 2 files and `lib` with 1.
- (DBT-21) Given an entry point `app/x.rb#create`, then it is listed under the folder `app` and links to the page of
  `app/x.rb`.
- (DBT-25) Given a folder of three files changed by 10, 4 and 0 commits, then it counts 10 commits.
- (DBT-25) Given a file of the run no commit of the history changed (not committed yet), then it counts 0 commits.
- (DBT-25) Given a doc item whose feature is no longer in the run, then it counts 0 commits.
- (DBT-23, DBT-24, DBT-29) Given a set apart use case with 0 commits and 0 entry points, then its priority is 0.25.
- (DBT-29) Given a use case scored 0.4, listed as low confidence and set apart for its confidence, then its low
  confidence item comes first (weight × weakness 0.6 against 0.25).
- (DBT-32) Given a use case validated then rewritten by a later run (its fingerprint differs), then it is listed
  again under the kinds of judgement it falls in.
- (DBT-32) Given a feature validated, when one of its source files is edited after the review, then it is listed
  again under the kinds of judgement it falls in, before any `generate`.
- (DBT-32) Given a validated use case that a section of the project's docs contradicts, or that is stale or set
  apart, then it is still listed under those kinds.
- (DBT-32) Given a validated domain, then its features and use cases are still listed under the kinds of judgement:
  a review vouches for its own section only.
- (DBT-30) Given a review of a section the run no longer holds, then no item is listed for it.
- (DBT-30) Given a section marked wrong whose fingerprint differs from the one of its review, then it is still
  listed as marked wrong, said outdated (DBT-37).
- (DBT-30, DBT-31) Given a review with no comment, then its item shows no reason.
- (DBT-19, DBT-20) Given a run with no saved behaviour coverage (not a `surface` run), then no item of the kinds
  "entry point without a feature" and "reached by no behaviour" is listed, and the page does not say so.

## Non-goals

- A validated section still counts in the low-confidence, unscored, unbacked and technical lists of `retrodoc report`
  and of `coverage-report.md`: they report the model's judgement; only the sections marked wrong are added to the
  report (DBT-35).
- No kind of its own for an outdated validation: the section is back under the kinds of judgement it falls in, and
  its page says the review is outdated (decided with the user on 2026-11-04).

- The entry points a feature holds but no use case cites, the features set aside as technical, the domains with no
  feature and the rules set aside are not items: `retrodoc report` lists them, and they are not code no doc covers
  nor a disagreement with the project's docs.
- The items `matched` by sections no verdict judged are not listed: the next `generate` judges them.
- No weight per size: a domain or a feature counts the entry points of all its features or use cases, so an
  undocumented domain ranks above its undocumented features: its doc is the first fix and covers theirs. Folding the
  items a larger one holds is not done.
- The weights per kind are fixed, not set in `retrodoc.toml`.
- The commits are those of the history when `retrodoc ui` starts, not when the run was made; a renamed file counts
  only the commits under its current name, as `scan` counts them.
- The page does not replace `retrodoc report` nor `coverage-report.md`, and calls no LLM (ADR 0039).
- The stale items are judged when the page is asked, the other items read once at start, like the other pages; the
  reconciliation is computed once at start from the saved verdicts, never judged again by the page.
