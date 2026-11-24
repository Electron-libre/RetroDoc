# 0056. The human corrections reach the passes as input of the command

Status: Accepted

## Context

The edits (ADR 0045) and the reviews (ADR 0044) of the docs are kept by `generate` but not read by it: a section a
human corrected, or marked wrong, is written again from the code alone, and the rewrite only makes the correction
outdated. The passes that write the sections (domains, features, use cases) live in `retrodoc-pipeline`, which does
not depend on `retrodoc-docs`, the owner of `.retrodoc/edits.json` and `.retrodoc/reviews.json`; `retrodoc-docs`
already uses the pipeline in its tests. Every input of a unit is part of its fingerprint (ADR 0005, ADR 0037), so a
correction put in it asks its unit again.

## Decision

`retrodoc-cli` loads the two files through `retrodoc-docs` at the start of `generate` and hands the passes plain
data, a type of `retrodoc-pipeline` (`Corrections`: per section, its edited fields and its review), with no new crate
edge. Each pass gives a unit the corrections of the sections it writes, in the request, by slug.

Only a review `wrong` asks its unit again, once: the reviews answered are recorded in `fingerprints.json` by section,
date and comment, and the fingerprint leaves the corrections out otherwise. An edit or a review `validated` guides
the next rewrite its unit gets for another reason, at no call of its own.

Rejected: `retrodoc-pipeline` depending on `retrodoc-docs` (a cycle with its tests, and the pipeline would read the
read model it is the writer of); moving the stores to `retrodoc-core` (they are the docs' reading side, ADR 0054); the
corrections in the fingerprint (an edit would ask its unit again at once, make itself outdated as "rewritten" by the
rewrite it caused, and a review changed back from `wrong` would ask again a unit nobody wants rewritten).

## Consequences

Amends ADR 0037 on one point: the corrections a request shows, and the fixed text that shows them, are not in the fingerprint (but the reviews
`wrong` answered), so a saved run may hold
answers written under other corrections; the measures name the edits and reviews of the run. A review `wrong` costs
one call of its unit (the domains call for a domain, which may change the domains of every feature). The model may
still rename a slug: the correction of a section it renames is left out by the readers (SED-24).
