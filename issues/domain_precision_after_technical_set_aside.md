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
