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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
A second judge call offers the generated items left unpaired after the first one against the reference items the
first call was not offered (the covered ones): the judged precision no longer falls as `matches.yaml` grows, and the
first call, so the judged recall, stays as it is. Out of scope: changing the first call, a single all-against-all call.

## Plan
1. [x] Measure: on the 12 saved hidden clones (f95ada6, dafea18), list the generated features and domains unpaired after the saved judgement whose right reference item is already covered, by hand, no LLM call; verified by a findings §10 entry and a ledger line; makes true the "measure first" of the issue. (autonomous)
2. [x] Second call: `judge` makes the second call per kind, records it in `judge.yaml`, `SCORING_VERSION` 5; verified by judge tests with a scripted LLM; makes true BSC-4, BSC-21. (autonomous)
3. [x] `--saved-judge --second-call` and the note of a `judge.yaml` without the second call; verified by CLI tests; makes true BSC-22. (autonomous)
4. [x] Baselines at scoring version 5: `--saved-judge --second-call` on the 12 clones (paid, cents, asked first), table against version 4, findings §10 and ledger; verified by the re-scored tables. (autonomous, paid run approved by the user)
5. [x] Docs and fixes of the `high` review of the issue; verified by `cardinal run project-check` and the BSC-22 test; makes true BSC-21, BSC-22 as amended. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 94956a25a14bb96cbf52b6c920320bb9100c3db7
- Re-judge the baselines with the second call alone on the saved `judge.yaml` (`--saved-judge --second-call`), not a whole new judgement.
- A second call that proposes no pair, twice, is warned about and does not mark the kind unjudged.

## Summaries

### 1. Measure
done: findings §10 counts the second pairs the offer hides (linkding 23 of 30, Lobsters 40 of 56 unpaired features
at f95ada6): judged feature precision 71% to 93% and 72% to 92% once paired; ledger line added. Spec BSC amended
(draft).

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass; draft warnings on BSC-10, 12, 14, 19 wording (unchanged requirements) |
| docs | findings §10 |
| code review | see below |

red: none (a measure, no test)
Code review: 1 finding
- the range of the bias read 14 to 21 points, 22 on linkding f95ada6: fixed.

not verified: the classing is one reader's; a part counted as a second pair is the judge's rule when offered, not checked by a judge call.
decisions needed: none

### 2. Second call
done: `judge` ends with `second_calls`: per kind, the generated items still unpaired against the reference items
`matches.yaml` covers, recorded in `judge.yaml` (`second_call`), skipped for an unjudged kind; `SCORING_VERSION` 5.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | BSC-4, BSC-21 traced |
| docs | `docs/crates/bench.md`, `docs/ARCHITECTURE.md`, findings §10 |
| code review | see below |

red: BSC-21: `assertion left == right failed, left: [], right: [Pair { generated: "User administration", reference: "Administration" }]`
Code review: 1 finding
- `second_calls` marked both kinds as run even for an unjudged kind it skipped: fixed, a kind is marked only when its call ran (asserted in the BSC-12 test).

not verified: no LLM run yet (deliverable 4).
decisions needed: none

### 3. `--second-call`
done: `benchmark --saved-judge --second-call` makes only the second calls the saved `judge.yaml` lacks and saves
them; `--saved-judge` alone prints a note naming the kinds without it. Spec BSC accepted.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | BSC-22 traced, spec `accepted` |
| docs | `docs/crates/render-mcp-cli.md`, skill `quality-benchmark` |
| code review | see below |

red: BSC-22: the test panicked at `before.contains("second call")`: the re-score printed no note.
Code review: no findings

not verified: no real LLM call yet (deliverable 4).
decisions needed: none

### 4. Baselines at version 5
done: second call run on the 12 hidden clones (~40 calls, ~$0.10); judged feature precision linkding 71% -> 92%,
Lobsters 72% -> 91% at f95ada6, recalls unchanged; findings §10 and ledger.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | unchanged |
| docs | findings §10 |
| code review | see below |

red: none (a measure)
Code review: no findings

not verified: the second call's pairs are not audited by hand (13 to 29 a run, counted in the reports).
decisions needed: none

### 5. Fixes of the `high` review
done: a report judged without the second call carries scoring version 4 and the table's hint names
`--second-call`; `second_calls` reuses the comparison and items of `judge`; a second-call pair equal once normalized
is not added again; BSC-21 says a rejected first-call proposal does not cover its item, BSC-22 the version 4 stamp, a
non-goal the rebuilt first offer; docs say `--second-call` makes LLM calls; findings figure 62% made 61%.

`high` review: 10 findings
- v5 stamped on a report without the second call: fixed (version 4).
- the first offer rebuilt from the current `matches.yaml`: not fixed, stated as a non-goal (the first offer is not saved).
- an empty second call asked twice: not fixed, the validated decision (BSC-10 holds for every call).
- the judgement lost when a second call fails: not fixed, as for the first calls and the narratives before it.
- a rejected first-call proposal leaves its item to the second call: kept, the spec says so.
- a second-call pair outliving the hand pair it relied on: not fixed, as any saved pair of the judge.
- "no LLM call" next to `--second-call`: fixed.
- 62% / 61%: fixed.
- `compare` and `RunItems::load` repeated: fixed.
- exact duplicate check: fixed (`same_as`).

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | BSC-21, BSC-22 amended, traced |
| docs | `docs/ARCHITECTURE.md`, `docs/crates/render-mcp-cli.md`, findings §10 |
| code review | see below |

red: BSC-22: the scoring assertion would have read 5 for a run without the second call (written with the fix).
Code review: no findings

not verified: none.
decisions needed: none
