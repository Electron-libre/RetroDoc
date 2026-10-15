# 0030. Ask the extraction passes at temperature 0, outside the fingerprints

Status: Superseded by 0031

## Context

Two runs on the same input gave different entry points, domains and features, and the spread was as large as
the effects the benchmark tries to measure. The requests left the sampling to the server. Measured on the
entry points pass alone (5 runs per setting, linkding and delivery_router, a local model and deepseek, see
`issues/done/sampling_temperature_for_steadier_runs.md`): at the server's default the entries all runs share were
2% to 16%, at temperature 0 they were 52% to 100%, the number of entries stopped swinging, and the share of
the reference found did not move in a consistent way. It is steadier, not deterministic, and not better.

## Decision

`llm.temperature` (also per pass) is sent as `temperature` when set. When unset, the extraction passes
(`EXTRACTION_PASSES`: sources, roles, business-files, glossary, entry-points, actors, confidence) are asked at
temperature 0 and the writing passes (brief, domains, features, use cases, rules) send nothing, so the
server's default applies: a low temperature may make narratives poorer, which the benchmark has not checked.
A server that refuses the parameter is asked again without it, as for the JSON schema, and the refused
parameter is left out for the rest of the run.

The temperature is not part of the fingerprints. Including it, like the model, would make the first run after
the default changed redo the glossary, the entry points and the actors, and through them the domains, the
features and the use cases: most of the cost of a run, for results that were fine. A saved unit stays as it
was; a unit derived again uses the new temperature; `generate --force` redoes everything. This is what the
prompts already do.

## Consequences

* Steadier re-derivations, so fewer runs per comparison. Not a guarantee: two runs at 0 still differ by 12% to
  28% except in one case.
* Only the entry points pass was measured. The other extraction passes get 0 by the same reasoning, to
  confirm with a measure of their own.
* A reasoning model may ignore the temperature; nothing is done about it.
* Results saved before this change were made at the server's default and are kept as they are.
* A user who wants the server's default for an extraction pass sets an explicit value.
