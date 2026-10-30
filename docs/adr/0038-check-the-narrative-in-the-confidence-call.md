# 0038. Check the narrative in the confidence call

Status: Accepted

## Context

Each use case has a business narrative above its steps (ADR 0009). The confidence pass scored the steps only
(ADR 0007, ADR 0026), and the business-language score says whether the narrative reads as business language, not
whether it is true. A hand measure on two benchmark applications (40 use cases, 200 claims) found 8 claims that no
code supports (a wrong condition, a wrong scope, an invented caller), 7 of them on the larger application, 3 of
which repeat a wrong step; and 23 more claims true only in code the use case does not cite. A mechanical check
(the entities and actors the narrative names must be in the steps or the cited code) caught 3 of these 31 claims
for 19 false alarms: the wrong claims are paraphrased effects and conditions, in words the steps already use.

## Decision

The narrative is checked by the model, in the request the confidence pass already makes for the doubtful steps: the
request shows it with the steps and the cited code, and the answer gives it a verdict like a step's. The verdict is
one more item in the mean of the use case score, and is kept apart on the use case (`narrative_check`) so the debt
report can list the narratives the code does not back. No mechanical check of the narrative's words.

## Consequences

A wrong narrative lowers its use case's score and is listed in the report, for no extra call when the use case
already went to the model. The confidence prompt changes, so every saved score is given again once (ADR 0037). A
use case whose steps all pass the checks keeps an unchecked narrative unless it is sampled. A narrative that
repeats a wrong step is not caught here.
