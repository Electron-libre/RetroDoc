# 0031. Ask the writing passes at temperature 0 too

Status: Accepted

## Context

ADR 0030 asked the extraction passes at temperature 0 and left the writing passes (brief, domains, features,
use cases, rules) at the server's default, because a low temperature might make narratives poorer. That was
measured (`issues/done/sampling_temperature_for_steadier_runs.md`): linkding, docs hidden, deepseek, 3 runs per
setting, only the writing passes differing. The share of narratives the judge rates in business language did
not fall (86% to 91%, ranges not overlapping), the use cases fully in business language did not move (72% and
71%), the number of features became steadier (50-89 to 67-73), cost and time were the same. The judged
recall and precision moved within the old ranges, and the audit of the judge's pairs of one run per series
found 4 wrong pairs of 21 at the default and 5 of 12 at 0 (the pairs left once equal names are matched), so
the judged figures say little about a difference between the two.

## Decision

Every pass is asked at temperature 0 when no temperature is set, but `contexts` and `repo-map` (not measured)
and the benchmark judge, which keep the server's default (`ZERO_TEMPERATURE_PASSES`). The rest of ADR 0030
stands: `llm.temperature` and `[llm.passes.<name>].temperature` set it, a server that refuses it is asked again
without it, and it is not part of the fingerprints. This supersedes the list of passes of ADR 0030.

## Consequences

* Steadier writing passes, so fewer runs per comparison. Not a quality gain: nothing measured says the docs
  are better, only that they are not poorer.
* One repository, one hosted model, 3 runs. A local model, a reasoning model and a second repository are not
  measured.
* Docs already written keep their results until `generate --force` (not in the fingerprints).
* `contexts` and `repo-map` can join the list once measured.
