---
priority: low
depends_on: [domain_precision_after_technical_set_aside]
why: 'a bias of the judged precision that an audit corrects; it misleads only a reader who skips the audit'
---
# Let the judge pair a generated item with a reference item already covered

# Goal
The judge is offered only the reference items no generated item covers (BSC-4), so a second generated domain or
feature that stands for an already covered reference item cannot be paired and counts as imprecise. The more pairs
`matches.yaml` holds, the fewer reference items are offered, and the judged precision of a new run falls while the
docs did not change. Expected: a judged precision that does not depend on how many pairs `matches.yaml` holds, or
the bias measured and stated next to the figure.

# Findings
* `domain_precision_after_technical_set_aside` (2026-11-04, f95ada6 hidden runs, deepseek-chat): judged domain
  precision 75% on linkding and 64% on Lobsters at the run, 96% and 86% once every domain pair was audited; the
  unpaired ones were second domains of covered reference domains (administration, integrations, private messages,
  hats, membership).
* The dafea18 re-scores keep the pairs their judge made when `matches.yaml` held fewer pairs, so the two commits
  were judged on different offers.
* The features judge has the same offer (`judge.rs`, `only(&reference_features, ...)`); its bias on feature
  precision is not measured.

# Approach
1. Measure first, no new LLM run: on the saved runs, count the unpaired generated features whose right reference
   feature was already covered (the hand audits of `docs/findings.md` §10 list the too fine ones).
2. Candidates: (a) offer every reference item to the generated items no pair covers yet, keeping the uncovered-only
   offer for recall; (b) keep the offer and report, next to the judged precision, how many reference items were
   not offered. Either changes BSC-4 and its non-goal, and bumps `SCORING_VERSION`; (a) needs the judge asked
   again on the kept runs (cents a run).

# Specs
* `specs/benchmark-scoring.md`: BSC-4 and the non-goal "Changing what the judge is shown or asked".

# Milestone
Measured quality at the current code

# Resources
* `crates/retrodoc-bench/src/benchmark/judge.rs` (`judge`, `propose_matches`), `matching.rs` (`compare`).
* `docs/findings.md` §10, the ledger lines of `domain_precision_after_technical_set_aside`.

# Hints
* A change of the judge makes the earlier judged figures incomparable: re-judge the baselines in the same issue.
