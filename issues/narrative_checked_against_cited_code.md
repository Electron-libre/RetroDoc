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
