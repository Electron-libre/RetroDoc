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
