---
priority: low
why: 'only the connectors to GitHub, GitLab and Jira, later than v1, need it; written so they start from a known shape'
---
# Let the non-code evidence come from sources outside the repository

# Goal
The non-code evidence of the brief and of `brief.evidence` (docs, commits, tests, schema, translations) is gathered
from the local repository only, into a closed list of kinds, and the per-unit retrieval has a fixed pool per kind.
The connectors of `PRODUCT.md` (issues, tickets, pull requests) would bring business vocabulary the domains lack.
Expected: a shape that lets a new source add its signals (and its pool and budget) without touching each pass, with
the signals of a remote source fingerprinted like the others.

# Findings
* 2026-11-07: `SignalKind` (`crates/retrodoc-ingest/src/signals/mod.rs`) is a closed enum of nine kinds, all read
  from the repository by `signals::collect`.
* 2026-11-07: `POOLS` (`crates/retrodoc-pipeline/src/brief/evidence.rs`) fixes three pools (documentation, tests,
  commits) with their budgets; a ticket kind would need a new pool there.
* 2026-11-07: the domain precision is the weakest benchmark figure (`PRODUCT.md`, Success measures); whether tickets
  would raise it is not measured.

# Approach
1. Write down what a remote source needs: its signals, their origin as a citable link, a snapshot for the
   fingerprints (a rerun must not call the network to stay a no-op), a budget in the prompts.
2. Candidates, to choose at the plan:
   1. new `SignalKind`s and a pool per kind, the snapshot saved under `.retrodoc/`;
   2. a source trait in `retrodoc-ingest` yielding signals, the repository being the first source.
3. No connector in this issue: the change keeps today's signals and prompts byte-identical.

# Resources
* ADR 0019 (product brief and non-code evidence), ADR 0036, ADR 0037.
* `specs/language-agnostic.md`, `docs/crates/core-ingest.md`.

# Hints
* Best done right before the first connector: the shape is best checked against a real source.
