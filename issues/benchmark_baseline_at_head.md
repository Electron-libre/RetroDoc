---
priority: high
why: 'the measured figures predate several changes of the passes; every quality issue of the milestone compares with this baseline'
---
# Measure a benchmark baseline at the current code

# Goal
The last whole benchmark runs date from 2026-10-28; since then the docs language, the narrative check in the
confidence call, the new rules judgement, the reconciliation by identifiers and many fixes landed. Nobody knows the
figures of the code as it is. Expected: one fresh baseline per benchmark application at the current commit, with its
spread, recorded in the ledger and in `docs/findings.md`, that the other issues of the milestone compare with.

# Findings
* Lobsters (2026-10-28, `deepseek-chat`, 1 run, confidence off): judged domains 100% recall / 82% precision, features
  100% / 51%, narratives in business language 81%, 126 calls, 9 min, about $0.3.
* linkding (2026-10-28, features replays on frozen domains, 3 runs): features found by hand about 75%, precision by
  hand about 77%; use cases replay (1421894): business 78%, score=1 75%.
* The G2 baseline of the domains pass on linkding is not saved (figures of 2026-10-23 lost).
* `delivery_router` saturates at 100%: a smoke repository, not a benchmark one.
* Since 0816a1c/7c64fe6 the system prompts are part of the fingerprints: a `generate` on a run saved before is a
  full run, not a rerun.

# Approach
1. Read the ledger: reuse any measure still `valid` at the current commit (`git diff <commit>..HEAD` on the pass).
2. With the user's go (estimate about $1: Lobsters about $0.3 a run, linkding less), one `just benchmark` series
   on each of linkding and Lobsters, hidden then shown, 2 to 3 runs, confidence on (the narrative check is new).
3. Re-freeze the inputs of both applications at that commit (`freeze_inputs`) if the surface passes changed, and
   replay the domains pass 3 times on linkding to restore its missing baseline (cents).
4. Write the figures with their spread in the ledger and a dated entry in `docs/findings.md`, and update the
   "Latest figures" paragraph of `PRODUCT.md`.

# Milestone
Measured quality at the current code

# Resources
* Skill `quality-benchmark`, the ledger of measures (outside git, see `CLAUDE.md`), `benchmark/linkding/`, `benchmark/lobsters/`.
* `issues/done/second_application_in_benchmark.md`, `issues/done/surface_domains_default_decision.md`.

# Hints
* Paid run: ask the user first, with the estimated cost.
* The judged precision is overstated (see `judge_overstates_feature_precision`): report the strict and judged figures
  side by side, and audit a sample of feature pairs by hand.
