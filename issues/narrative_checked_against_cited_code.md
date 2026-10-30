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
3. [ ] Measure: an example runs the confidence pass on the 40 sampled use cases with DeepSeek (cents, asked before)
   and counts the 8 known U claims flagged and the narratives flagged wrongly; figures in the issue and the ledger.
   ~150 lines.

## Decisions
- Mode: autonomous
- Base commit: a006efc
- Narratives of use cases the checks settle: not checked, no extra call (only the `--confidence-sample` ones are).
- Weight of the narrative verdict: one more item in the mean, like a step.

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
