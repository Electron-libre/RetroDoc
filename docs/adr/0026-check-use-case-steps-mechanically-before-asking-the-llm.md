# 0026. Check use case steps mechanically before asking the LLM

Status: Accepted

## Context

The confidence pass asked the LLM for a verdict on every step of every use case: about a fifth of the
calls of a run (ADR 0020 step 7). Most of what it verifies can be read without a model: the cited lines
exist, the identifiers a step names are at those lines, the entities it names are in the cited code. A first measure on a small Ruby library said 93% of the steps passed and 69% of the use cases were settled
by the checks, but a review showed it counted a step that names no identifier and no entity as passing
because its cited lines exist, which proves nothing. Such a step is now doubtful. Measured again on the
same library (13 use cases, 75 steps, local model): 80% of the steps pass, 20% are doubtful, none broken;
6 of the 13 use cases (46%) are settled by the checks alone, on the same side of the 50% flag as the model
in every case, with a mean gap of 0.25 (the model is stricter). The pass made 4 calls instead of 8.

## Decision

Each step is checked first (`confidence/checks.rs`): `Pass` is supported and `Broken` (unreadable file, line
out of range) is unsupported, without a call; a step citing no code stays capped at 0.25 without a call.
Only `Doubtful` steps (a named identifier or entity is not in the cited code, or the step names none to check) are sent to the LLM, with
their code alone. `--confidence-sample N` keeps its cost-control role in the other direction: N use cases
that pass every check also get a full LLM opinion. Identifiers are extracted from the step text by
heuristic (backticks, `snake_case`, `CamelCase`, calls); a structured field in the model was rejected,
since it would change `Step` and invalidate every cache. The score scale and the 50% flag do not change.

Mechanical scores are given to use cases without a score and not recomputed on every run: recomputing
would overwrite the second opinions and break the no-op rerun. A new model clears all scores; the
mechanical ones come back for free.

## Consequences

The pass makes fewer calls (4 instead of 8 on the small library). Scores of passing use cases are
somewhat optimistic (the model marks `partial` where the code does not show a detail); the sample measures
that gap (`examples/confidence_agreement.rs`). A step whose cited lines are wrong but whose names appear
elsewhere in the file is `Doubtful`, not `Pass`, so a wrong citation is sent to the model rather than
trusted. Not measured on a large repository.
