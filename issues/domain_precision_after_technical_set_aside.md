---
priority: medium
depends_on: [feature_precision_on_applications]
why: 'domain precision is now the weakest benchmark figure, and a domain page with no feature is noise the reader meets first'
---
# Explain the fall of the domain precision and drop the domains without a feature

# Goal
The whole benchmark of `feature_precision_on_applications` (f95ada6, 2026-11-03) gave a judged domain precision of
75% on linkding and 64% on Lobsters, against 100% and 85% at dafea18 with the same `matches.yaml`, while the domains
pass is unchanged since dafea18. linkding gave 8 domains each run (6 at dafea18), one of them with no feature. A
domain whose features are all set aside as technical also keeps a page with no feature. Expected: the cause of the
fall known, and no domain page, report row or index entry for a domain with no feature.

# Findings
* f95ada6 hidden runs, 3 per application: linkding domains 8, 8, 8, with `feeds` (run 1) and `import-export` (run 2)
  holding no feature and no feature set aside; Lobsters 8 to 11. Judge's domain pairs not audited: 3 (2-5) on linkding,
  6 (2-8) on Lobsters.
* `git diff dafea18..f95ada6` is empty on the domains, surface, entry points and brief passes.
* The `high` review of `feature_precision_on_applications`: `domain_models` and the render loop over every domain of
  `domains.yaml`, so a domain left with only technical features is published empty.

# Approach
1. Measure first, no LLM: audit the domain pairs of the f95ada6 runs (the unaudited ones), and compare the
   `domains.yaml` of the dafea18 and f95ada6 clones (same inputs? same entry points?).
2. Candidates: (a) leave a domain with no feature out of the docs, the index and the report's domain table, listed in
   the debt report instead; (b) if the domains pass itself drifts (hosted model), replay it x3 on the frozen inputs
   (cents) against the dafea18 replays.

# Milestone
Measured quality at the current code

# Resources
* `docs/findings.md` §9 and §10, the ledger lines of `feature_precision_on_applications`.
* `crates/retrodoc-pipeline/src/report/mod.rs` (`domain_models`), `crates/retrodoc-render/src/markdown/mod.rs`.

# Hints
* The domain precision of the judge rests on unaudited pairs: audit before reading a fall into it.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
The fall is mostly a scoring artifact, not a drift of the domains pass (no LLM, from the saved f95ada6 and dafea18
runs): BSC-4 offers the judge only the reference domains no generated domain covers, and `matches.yaml` gained
domain pairs after the dafea18 runs were judged (a9b80d8, 482ddc3), so a second generated domain of an already
covered reference domain (linkding `Administration`, `Appearance and Integrations`; Lobsters `Private messaging`,
`Hat identities`, `Membership and accounts`) can no longer be paired, while the dafea18 re-scores keep the pairs
their judge made on a fuller offer. The other unmatched ones are the domains with no feature (linkding `feeds`,
`import-export`: entities only, no resource, nothing set aside) and Lobsters `Site operations`, unmatched at dafea18
too. Hand reading of the unmatched domains: linkding 88%, 88%, 100% (8/8 with the too-fine `Import and Export`
paired to Bookmark management), Lobsters 90%, 89%, 82%, against 100% and 85% at dafea18; the 8 domains of linkding
are within the dafea18 range (5 to 9 over both series). Out of scope: changing what the judge is offered
(non-goal of BSC, a follow-up issue) and the domains pass itself (candidate b, not needed).

## Plan
1. [x] Audit: the domain pairs read by hand added to `benchmark/{linkding,lobsters}/matches.yaml` (pairs and
   rejected pairs), `--saved-judge` re-score of the f95ada6 runs (no LLM), finding in `docs/findings.md` §9, ledger
   line; verified by `just check` and the re-scored table; ~80 lines. (autonomous)
2. [x] Domains with no feature left out of the docs: one filter of the published domains used by `domain_models`
   (render, MCP, web), the agent index and the reconciliation items, the report lists them in a section of its own;
   verified by tests through the binary of EDM-1, EDM-2, EDM-5, EDM-6, internal for EDM-3, EDM-4; makes true EDM-1
   to EDM-6; ~350 lines. (autonomous)
3. [x] The benchmark counts the published domains only (matching, judge offer, metrics), `SCORING_VERSION` 4;
   verified by a test of BSC-20 and a `--saved-judge` re-score of the f95ada6 and dafea18 hidden runs (no LLM);
   makes true BSC-20 (amended in `specs/benchmark-scoring.md` with this deliverable: an
   accepted spec cannot hold an untraced requirement); ~150 lines. (autonomous)
4. [x] Follow-up issue (`skill:create-issue`): the judge cannot pair a generated item with a reference item
   already covered (BSC-4), so the judged precision falls as `matches.yaml` grows. (autonomous: `issues/judge_offer_hides_second_pairs.md`)
5. [x] Fixes of the `high` review of `4e27282..HEAD`: the number of domains follows the scoring version (BSC-7
   amended); a `features.yaml` present but unreadable scores no feature, with a warning, not every domain;
   `published` and `featureless` share one set of slugs; the section of the domains with no feature written with
   the core sections, not the `surface` ones; the EDM-6 test reads the domain table itself. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 4e272829561bf1de395be18ae8283b917fc6cd62
- The judge's offer (BSC-4) is left as it is: a follow-up issue (deliverable 4).
- The benchmark counts the published domains only (deliverable 3, scoring version 4).
- A too-fine domain of one feature (linkding `Import and Export`) is paired with its parent reference domain.

## Summaries

### 1. Audit
- done: every domain pair of the 12 hidden runs (f95ada6, dafea18) read by hand into `matches.yaml` (37 pairs, 3
  rejected), finding in `docs/findings.md` §10, ledger line; audited domain precision linkding 95% -> 96%, Lobsters
  88% -> 86%: no fall.
- checks: `just check` ok; `--saved-judge` re-score of the 12 runs (`~/retrodoc-runs/domain_precision_after_technical_set_aside/`).
- red: none (a measure, no test).
- Code review: no findings
- not verified: the feature pairs (not in scope).
- decisions needed: none.

### 2. Domains with no feature left out of the docs
- done: `DomainMap::published` / `featureless` (pipeline); `domain_models` (render, MCP, web) and the
  reconciliation items keep the published domains only; the debt report has a section "Domains with no feature"
  with the features set aside of each; spec `featureless-domains` accepted; scenario WEB-4 amended (a domain with
  no feature is not listed); the web and MDR fixtures that relied on a domain with no feature give it one.
- checks: `just check` ok; docs `docs/crates/pipeline.md`, `docs/crates/render-mcp-cli.md`.
- red: EDM-1, EDM-2: `assertion failed: !functional.join("feeds").exists()`; EDM-5, EDM-6: no "## Domains with no
  feature" in the report; EDM-3, EDM-4: written after the filter, not seen red.
- Code review: no findings
- not verified: a real run (no LLM call in this deliverable).
- decisions needed: none.

### 3. The benchmark counts the published domains only
- done: `scored_domains` (bench): the matching, the judge's offer and the domain count of `RunMetrics` keep the
  domains a feature belongs to, every domain when the run saved no features (domains replay); `SCORING_VERSION` 4;
  BSC-20 added; re-score of the 12 hidden runs at version 4: linkding f95ada6 100% (7 domains in two runs), Lobsters
  unchanged; finding and ledger line.
- checks: `just check` ok; docs `docs/crates/bench.md`, `docs/findings.md`.
- red: BSC-20: `Run: 2 domain(s), 1 feature(s)` printed, and the judge's prompt listed `- Feeds:`.
- Code review: no findings
- not verified: none.
- decisions needed: none.

### 5. Fixes of the high review
- done: the review fixes listed in the plan.
- checks: `just check` ok.
- red: BSC-7: `| Domains | 3 | — | 1 | 3 | 0 |` shown against an older scoring version.
- Code review: no findings
- not fixed, with the reason: the `|` of a domain name in the report table (the other tables of the report don't
  escape either: a matter for all of them); the second load of the artifacts by the judge after `compare` and one
  view of the published domains for every reader (refactors outside the requirements; the readers that render or
  count domains all go through `published` now).
- not verified: none.
- decisions needed: none.
