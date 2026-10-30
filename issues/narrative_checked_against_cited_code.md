---
rank: 100
why: 'last open issue of the earliest open milestone (Business-level documentation, `docs/findings.md`, 2026-10-30): the narrative is the first thing a reader trusts and nothing checks it; its measure reads saved benchmark runs'
---
# Check the business narrative of a use case against the code it cites

# Goal
Each use case has a business narrative ("What happens") above its folded technical steps. The confidence pass scores
the steps only (mechanical checks, then the LLM for the doubtful ones), and the business-language score only says
whether the narrative speaks code or names nothing of the business. A narrative can therefore claim an effect (a
notification sent, a signature cancelled) that no step and no cited code supports, at full confidence. Expected: a
claim of the narrative that the steps or the cited code do not back lowers the score or is flagged in the report.

# Findings (code read on 2026-10-30)
* `crates/retrodoc-pipeline/src/confidence/mod.rs` checks each step ("the cited lines exist, the identifiers and
  entities it names are in the cited code"); no line of the pass reads `narrative`.
* `docs/findings.md` §2 step 4d: "The narrative is not checked against the code (the confidence pass still scores the
  steps only)"; step 4e made the business-language score, which cannot tell a wrong narrative from a right one.
* Not measured: how often a narrative states more than its steps on the benchmark runs.

# Approach
1. Measure first: on saved benchmark runs, compare by hand a sample of narratives with their steps and cited code,
   and count the unsupported claims.
2. Candidates: (a) mechanical: the entities and actors the narrative names must appear in the steps or the cited
   code; (b) the narrative joins the doubtful items the confidence call already makes (no extra call when the steps
   pass); (c) a report section for the use cases whose narrative names what their steps do not.

# Milestone
Business-level documentation

# Resources
* `crates/retrodoc-pipeline/src/confidence/`, `crates/retrodoc-pipeline/src/vocabulary.rs`.
* ADR 0007 (confidence by cross-check), ADR 0009 (narrative and vocabulary score), ADR 0026 (mechanical checks first).
* `skill:quality-benchmark`; the ledger of measures before any paid run.

# Hints
* The narrative is meant to stay above the code (no class or method names): the check must match business words, not
  identifiers.

# Tracking

Plan: validated

## Understanding
The confidence call that already checks the doubtful steps also judges the narrative against the cited code; its
verdict counts in the score, is kept on the use case and listed in the debt report. Out of scope: a mechanical word
check (measured useless below), narratives repeating a wrong step, steps citing the wrong lines.

## Measure (2026-10-30, saved runs, no LLM call)
By hand, 20 use cases of linkding (`brief_injection_experiments-1421894/evidence`, run 1, every 4th) and 20 of
lobsters (`second_application_in_benchmark-d8d8c92-1`, every 20th); each narrative split into claims: stated by a
step (S), at the cited lines (C), true elsewhere in the tree (F), unsupported or wrong (U).

| repo | use cases | claims | S | C | F | U | use cases with a U | mechanical check: catches of F+U | false alarms |
|---|---|---|---|---|---|---|---|---|---|
| linkding | 20 | 104 | 75 | 13 | 15 | 1 | 1 | 1 of 16 | 14 |
| lobsters | 20 | 96 | 77 | 4 | 8 | 7 | 7 | 2 of 15 | 5 |

* U claims: a wrong condition or reason ("so the job is retried", retry is commented out), a wrong scope ("honoring
  their tag filters", "newest" for the hottest list, "template" files watched), an invented caller, an over-reaching
  definition (hats worn "on a story"). No invented notification. 3 of the 7 lobsters U repeat a wrong step.
* F claims are mostly harmless (outcomes restated, facts just outside the cited lines); two drop a condition (saved
  only on "save", superuser only).
* Mechanical check (candidate a: glossary entities and actors of the narrative in the steps or the cited files):
  3 catches of 31 F+U, 19 false alarms (actor names found in the step's actor field only, common words such as
  Link, Member, Message). Rejected.
* Seen on the way, out of scope: steps citing the wrong lines while their names pass (lobsters `disown-comment`,
  `send-pushover-notification`; linkding `create-bookmark` step 1).

## Plan
1. [x] Narrative in the confidence call: the request shows it, the answer's verdict counts in the score, named in
   the rationale, kept in `UseCase::narrative_check` (spec `use-case-confidence`, ADR 0038 draft); verified by
   pipeline tests with a recording fake LLM; makes true CNF-1 to CNF-7. ~450 lines.
2. [x] Report section "Narratives the cited code does not back"; verified by a report test; makes true CNF-8, spec
   `accepted`. ~120 lines.
3. [x] Measure, then the narrower verdict: an example runs the confidence pass on the 40 sampled use cases with DeepSeek (cents, asked before)
   and counts the 8 known U claims flagged and the narratives flagged wrongly; figures in the issue and the ledger;
   then `unsupported` only for a claim the code contradicts, the report listing those only (CNF-9), measured again.
4. [x] Docs and fixes of the `high` review of the issue: the narrative is judged against the code of every step
   (CNF-10), each use case of a batch says what it cites, an unknown or misshaped narrative verdict is none (CNF-11,
   CNF-12), `Answer` folded into `RawVerdicts`, the examples' docs and table cells; findings.md and ADR 0038.
   ~150 lines.

## Decisions
- Mode: autonomous
- Base commit: a006efc
- Narratives of use cases the checks settle: not checked, no extra call (only the `--confidence-sample` ones are).
- Weight of the narrative verdict: one more item in the mean, like a step.
- After the measure of deliverable 3 (2026-10-30): `unsupported` is kept for a claim the shown code contradicts, a
  claim it merely does not show is `partial`; the report lists the `unsupported` narratives only (CNF-8 withdrawn,
  CNF-9); measured again on the same samples.

## Deliverable 1 summary
done: the confidence request shows the narrative, its verdict is one more item of the mean, named in the rationale
and kept in `UseCase::narrative_check`; ADR 0038 accepted; ARCHITECTURE, pipeline and findings docs updated.

| check | result |
|---|---|
| tests | `just check` pass |
| lint | clippy pedantic clean |
| specs | CNF-1 to CNF-7 traced (internal: faked model) |
| docs | updated |
| code review | low, no findings |

red: CNF-1, CNF-3, CNF-4, CNF-5: `assertion failed: prompt.contains(NARRATIVE) && prompt.contains("=== a.rs ===")`
(CNF-2, CNF-6, CNF-7 are guards: they passed before the code).
Code review: no findings (its one doubt, the narrative dropped by `doubtful_part`, is ruled out: it clones the use case, the tests see the narrative in the prompt).
not verified: how often the model flags a narrative (deliverable 3).
decisions needed: none.

## Deliverable 2 summary
done: the debt report lists the use cases whose narrative is judged partial or unsupported, with the reason, in
"Narratives the cited code does not back"; spec `use-case-confidence` accepted; pipeline and ARCHITECTURE docs.

| check | result |
|---|---|
| tests | `just check` pass |
| lint | clippy pedantic clean |
| specs | CNF-8 traced (internal: report built from cached artifacts), spec accepted |
| docs | updated |
| code review | low, no findings |

red: CNF-8: `assertion failed: md.contains("## Narratives the cited code does not back")`
Code review: no findings
not verified: nothing beyond deliverable 3.
decisions needed: none.

## Deliverable 3 measure (2026-10-30, 280641f + example, deepseek-chat, ~$0.02)
`examples/narrative_check_measure.rs` on the 40 use cases labelled by hand above; runs 1-2 as `generate` (only
the use cases with a doubtful step go to the model), runs 3-4 with `--all` (every sampled use case).

| run | repo | narratives checked | judged partial | judged unsupported | hand U claims caught with their reason | calls | prompt tokens |
|---|---|---|---|---|---|---|---|
| 1 | linkding | 15 of 20 | 13 | 0 | 1 of 1 (templates not watched) | 12 | 16.9k |
| 2 | lobsters | 15 of 20 | 9 | 0 | 1 of 7 (front page is the hottest list) | 13 | 14.9k |
| 3 | linkding `--all` | 20 | 15 | 0 | 1 of 1 | 18 | 31.6k |
| 4 | lobsters `--all` | 20 | 13 | 1 | 1 of 7 | 19 | 26.0k |

* The model flags about two narratives in three as `partial`, almost always for a claim outside the cited lines
  (an outcome restated, a fact of the uncited code: the F claims of the hand measure), which it cannot tell from
  an invented one. The two caught are the claims the shown code contradicts.
* Missed: wrong conditions and reasons ("so the job is retried", hat recreated "because it cannot be edited"),
  over-reaching definitions; two lobsters use cases with a U are flagged for another claim.
* The one `unsupported` (lobsters `send-pushover-notification`) is a step citing the wrong lines.
* As shipped, the report section would list 65% of the narratives, the score loses ~0.1 on each.

## Deliverable 3, measured again with `unsupported` only for a contradicted claim (runs 5-8, ~$0.02)

| run | repo | judged partial (before) | judged unsupported | listed by the report | hand U claims caught with their reason | calls | prompt tokens |
|---|---|---|---|---|---|---|---|
| 5 | linkding | 9 (13) | 0 | 0 | 1 of 1, as `partial` | 12 | 17.3k |
| 6 | lobsters | 5 (9) | 0 | 0 | 0 of 7 (front page: "newest not visible", as `partial`) | 13 | 15.4k |
| 7 | linkding `--all` | 11 (15) | 0 | 0 | 0 of 1 | 18 | 32.2k |
| 8 | lobsters `--all` | 6 (13) | 1 | 1 | 0 of 7 | 19 | 26.7k |

* Fewer narratives lose score for a claim outside the cited lines (about one in three instead of two in three).
* The model gives `unsupported` to no contradicted claim: the templates claim is `partial`. The one `unsupported`
  is still the step citing the wrong lines (`send-pushover-notification`).
* So the report section stays empty on these samples but for that case: the check catches no wrong narrative there.

## Deliverable 3 summary
done: `examples/narrative_check_measure.rs`; the confidence prompt keeps `unsupported` for a narrative claim the
shown code contradicts; the report lists the `unsupported` narratives only, section "Narratives the cited code
contradicts" (CNF-8 withdrawn, CNF-9); ADR 0038, ARCHITECTURE and pipeline docs.

| check | result |
|---|---|
| tests | `just check` pass |
| lint | clippy pedantic clean |
| specs | CNF-9 traced (internal: report built from cached artifacts), spec accepted |
| docs | updated |
| code review | low, 1 finding |

red: CNF-9: `a partial narrative is not listed`
Code review: 1 finding: the `score` of the listed narratives is always 0 (not fixed: they reuse `WeakSection`, shared with the other lists; a type of their own for one dead field is not worth it).
not verified: a run on a whole application.
decisions needed: whether to keep the narrative check, given the figures above (asked at the end of the issue).

## Deliverable 4 summary
done: fixes of the `high` review (10 findings) and the docs of the whole issue.

| check | result |
|---|---|
| tests | `just check` pass |
| lint | clippy pedantic clean |
| specs | CNF-10 to CNF-12 traced (internal: faked model), spec accepted |
| docs | findings.md 4d, ADR 0038 consequences, pipeline.md |
| code review | high on the issue (10 findings), low on the fixes (no findings) |

red: CNF-10, CNF-11, CNF-12: `prompt.contains("=== a.rs ===")` false; unknown verdict scored 0.8, not 1.0; misshaped
narrative left the use case unscored.
High review: 10 findings. Fixed: narrative judged against the doubtful steps' code only (CNF-10); an unknown verdict
reported as a contradiction (CNF-11); a misshaped narrative losing the step verdicts (CNF-12); a batch narrative
judged against a sibling's code (each use case now says what it cites); `Answer` duplicating `RawVerdicts`;
`confidence_agreement` comparing scores that now hold the narrative (doc note); `--all` doc; table cells of the
example. Not fixed: `partial` lowers only the use cases sent to the model (the weight the user chose); the harness
line of `docs/dev/agent-harness.md` swept into 2d674bb (left as is, the user's call).
Code review: no findings (low; its doubt, `Cites:` glued to the narrative, is ruled out: the narrative line ends with a newline).
not verified: the measure was made before CNF-10 (narrative shown the doubtful steps' code only); the share of
`partial` may drop with the whole code shown.
decisions needed: whether to measure again after CNF-10 (cents).
