---
priority: low
depends_on: [human_edits_and_reviews_guide_the_next_run]
why: 'the estimate understates a run guided by reviews; the calls it misses are few and cheap on a rerun'
---
# Count in the estimate the units a review marked wrong asks again

# Goal
A review `wrong` not yet answered asks its unit again at the next `generate` (HCR-4), but the estimate printed before
the first call does not know it: it announces no call for a rerun that makes some. Expected: the estimate counts the
units such a review asks again, and the calls they cascade into where it already counts a cascade.

# Findings
* 2026-11-24, default smoke repository, deepseek-chat (`docs/findings.md` §15): a rerun with two reviews `wrong` (a use
  case, a domain) made 13 calls; the estimate said every pass was within 0 calls, then reported each pass "above 0".

# Approach
1. Read the pending reviews in the estimate as the passes do (`corrections::pending`, `Fingerprints::answered`), the
   corrections loaded before the estimate is printed.
2. Count one unit per pending review: the domains call, the features call of a domain's slice, the use cases call
   of a feature; then the passes downstream as the estimate counts a changed unit today.

# Specs
* `specs/run-estimate.md`: a requirement for the units a review `wrong` asks again; `specs/human-corrections.md`.

# Milestone
Read by someone taking an application over

# Resources
* `crates/retrodoc-pipeline/src/estimate.rs`, `crates/retrodoc-pipeline/src/corrections.rs`,
  `crates/retrodoc-cli/src/commands/generate/estimate.rs`, ADR 0056.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-25

## Understanding
The estimate printed before the first call counts the units a review `wrong` not yet answered asks again (domains
call, features call of a domain, use cases call of a feature) at the measured cost per unit of their pass, plus the
rules and confidence of the rewritten feature; no cascade below a domains call, nothing for reconcile (EST-12 to 14).

## Plan
1. [x] Pipeline: `corrections` tells the units a pending review asks again (`ReviewedUnits`: domains, domains of
   reviewed features, features of reviewed use cases, from `Fingerprints::answered`), and `estimate` costs them per
   unit and cascade, in a new module `estimate/reviews.rs` (estimate.rs is at 625 lines); verified by internal tests
   of EST-12 and EST-13; ~350 lines. (autonomous)
2. [x] CLI: `generate --estimate` loads the corrections (edits.json refused as HCR-11), the CLI estimate gives the
   pending units and the saved use cases per feature; verified by binary tests EST-H1, EST-12 (answered review at
   zero), EST-14; docs (`docs/crates/pipeline.md`, `docs/findings.md` §15 note); ~250 lines. (autonomous)
3. [x] Fixes of the `high` review: one-slice domains (`<domain>/-`) answered, a feature with no entry point not
   counted, a call a review asks at least once, `--confidence-sample` kept on a reviewed confidence, the CLI test
   independent of the table, the contexts path documented; verified by internal tests of EST-12, EST-6. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 6ef92ae6ffef9c1706eb7363c5813d61bd633f94
- A domain review counts the domains call only, no cascade (1 a).
- Features counted per domain holding a reviewed feature, not per slice (2 a).
- `--estimate` fails on an unreadable `edits.json`, as `generate` (3 a).

## Summary of deliverable 1
- done: `ReviewedUnits` (pending reviews read from the corrections and `Fingerprints::answered`) costed per unit in
  `estimate`, `Basis::Reviewed`; spec amended (EST-12 to EST-14, EST-H1); pipeline doc.
- checks: tests ok, lint ok, specs ok (draft), docs ok, code review 1 finding.
- red: EST-12, EST-13: the four estimate tests left every pass at zero (`left: [("domains", 0, 0), ...]`).
- Code review: 1 findings: the CLI passes `ReviewedUnits::default()`: deliberate, deliverable 2 wires it.
- not verified: the binary path (deliverable 2).

## Summary of deliverable 2
- done: `generate` and `generate --estimate` read the corrections and give the estimate the units pending reviews ask
  again (none with `--force`); spec `accepted`; README, CLI and pipeline docs, findings §15 note.
- checks: tests ok, lint ok, specs ok (accepted), docs ok, code review no findings.
- red: EST-H1, EST-12, EST-13, EST-14: the estimate of a rerun with a use case marked wrong read `LLM estimate: 0
  call(s)`; `generate --estimate` succeeded on an unreadable `edits.json`.
- Code review: no findings
- not verified: a paid rerun on the smoke repository (the 13 calls of findings §15 against the new estimate).

## Summary of deliverable 3
- done: the findings of the `high` review fixed (see plan line 3); spec EST-12 and a non-goal on the contexts path.
- checks: tests ok, lint ok, specs ok, docs ok, code review ok.
- Code review: no findings
- red: EST-12, EST-6: `left: ReviewedUnits { feature_domains: 1, .. }` for a review answered under `orders/-`;
  `left: ("features", 0, 2)`.
- High review: 7 findings: 1, 2, 5, 6, 7 fixed; 3 (contexts path counts the reviews of every context) stated as a
  non-goal: the slice is planned from the context map after the estimate; 4 (nit, an answer kept under the old
  domain of a moved feature) not fixed: a feature that moves domain changes its print, so the run asks it anyway
  and the estimate counts it on its changed files.
- not verified: a paid rerun on the smoke repository.
