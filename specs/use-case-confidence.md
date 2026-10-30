---
id: CNF
status: accepted
adrs: [0007, 0026, 0038]
---

# Use case confidence

## Intent

The confidence pass tells the reader how far a use case is backed by the code it cites. This spec covers the
business narrative ("What happens") of a use case: the claims it makes are checked against the cited code in the
same call as the doubtful steps, so a narrative that states what the code does not show lowers the score and is
listed in the debt report.

## Requirements

- **CNF-1** (deterministic) A request of the confidence pass that holds a use case with a narrative must show that narrative next to the use case's steps and cited code.
- **CNF-2** (deterministic) The confidence pass must not make a request for a use case whose steps the mechanical checks settle, unless it is one of the `--confidence-sample` second opinions.
- **CNF-3** (deterministic) The narrative verdict of the model must count as one more item in the mean of the use case score, valued like a step verdict (`supported` 1, `partial` 0.5, anything else 0).
- **CNF-4** (deterministic) A narrative judged `partial` or `unsupported` must be named in the rationale of the use case score, with the model's reason.
- **CNF-5** (deterministic) The use case must keep the narrative verdict apart from its score (`narrative_check`), so that a narrative never checked has none.
- **CNF-6** (deterministic) An answer that gives no verdict for the narrative must leave the narrative out of the score and of `narrative_check`, the steps scored as before.
- **CNF-7** (deterministic) A use case without a narrative must be scored from its steps alone, its request showing no narrative.
- **CNF-8** (deterministic) The debt report must list, in a section of its own, each use case whose narrative was judged `partial` or `unsupported`, with the model's reason.

## Scenarios

- (CNF-2) Given a use case whose steps all pass the checks and no `--confidence-sample`, when the pass runs, then no
  request is made for it and its `narrative_check` stays empty.
- (CNF-3) Given four steps that pass and a narrative judged `unsupported`, when the use case is scored, then its score
  is 0.8.
- (CNF-6) Given a batched answer whose entry for a use case holds step verdicts but no narrative verdict, when it is
  scored, then the score is the mean of the steps alone.
- (CNF-8) Given no narrative judged `partial` or `unsupported`, when the report is written, then it has no such section.

## Non-goals

- A mechanical check of the narrative's words against the steps or the cited code: measured on two benchmark
  applications (`issues/narrative_checked_against_cited_code.md`), it caught 3 of 31 claims not at the cited lines
  for 19 false alarms.
- Catching a narrative claim that repeats a wrong step: the step check is where that is caught.
- Restating the step checks of ADR 0026 here.
