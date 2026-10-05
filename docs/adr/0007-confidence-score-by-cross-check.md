# 0007. Confidence score from an LLM cross-check against the cited code

Status: Accepted

_Retroactive ADR, reconstructed from the history (c1530d0, 09e00fe, 5661691; 2026-10-01 to 2026-10-03)._

## Context

Generated documentation is only useful if the reader knows what to trust (`PLAN.md` §1, "Reliability").
A generator that grades itself in the same call is not a check.

## Decision

A separate pass asks the LLM, for each step of a use case, a verdict `supported`/`partial`/`unsupported`
against the code excerpts the step cites. Steps with no readable cited code are capped at 0.25 instead
of being trusted. Scores aggregate from steps to use cases to features and to the whole repo, and
feed a documentation debt report (`retrodoc report`, reads the saved artifacts, no LLM call): overall
confidence, low-confidence sections, unscored use cases, undocumented code. The pass is optional and
bounded in cost: `--no-confidence` skips it, `--confidence-sample N` scores N unscored use cases spread
evenly (the rest stay unscored for later runs), and use cases of one feature are batched in a call.

## Consequences

Every section carries a measurable, comparable score. The score rewards closeness to the code, which
favours literal paraphrase over business meaning: this is why 0009 adds a separate business-language
score. It costs one more LLM call per step group, hence the sampling option.
