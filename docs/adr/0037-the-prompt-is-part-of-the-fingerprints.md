# 0037. The prompt is part of the fingerprints

Status: Accepted

## Context

ADR 0005 left the prompts out of the fingerprints: a prompt change redoes nothing until `--force`. The passes
then drifted apart: the rules fingerprint holds its three system prompts, the reconciliation a hand-bumped
version of what it shows (`SHOWN_RULE`), the others (reading passes, domains, features, placement, use cases,
actors, confidence) only their input and model. A saved run keeps the answers of the old prompt, so a measure of
a prompt change on a saved run compares the old answers with themselves unless its cache is cleared by hand
(found while measuring `issues/done/joint_reading_routing_rule.md`), and a saved run mixes answers of several
prompts without saying so.

## Decision

Every LLM pass that reuses saved results puts the hash of its system prompts (constants of the binary; the
reading passes add the fixed text of their user prompts, one part for the three since the joint reading saves
what the two others reuse) in what it compares for each unit, next to the model (`fingerprints::prompt_part`).
The rules pass keeps the hash of its prompts it already had, the reconciliation keeps `SHOWN_RULE` beside it
(it covers what is shown, not the prompt). The fixed text of the other user prompts is left out (it is spread
in `format!` calls; the instructions live in the system prompts): a change to that text alone still needs
`--force`. A changed prompt redoes the units of that pass once; the same binary gives the same print, so a rerun
with no change still makes no call. The hand-editable artifacts (`roles`, `sources`,
`business-files`, `brief`, `contexts`) stay until `--force`, as an edit by hand would.

Rejected: a version constant per pass bumped by hand (`SHOWN_RULE`): a cosmetic edit would cost nothing, but a
forgotten bump is the silent stale cache this decision removes. Rejected too: keeping the prompt out and giving
the measures a flag to ignore the cache of a pass: the saved run would still mix prompts.

Amends ADR 0005 on that point (its "changing a prompt does not invalidate caches by itself").

## Consequences

A measure of a prompt change on a saved run needs no cleaning. The first run after this change, and after each
prompt edit, asks the units of the edited pass again: on a large repository a reading-pass edit is a full
re-read, to count in the measure budget. A cosmetic edit of a prompt costs the same as a real one.
