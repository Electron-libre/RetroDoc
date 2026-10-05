# Architecture Decision Records

One file per architectural decision, named `NNNN-short-title.md` (next free four-digit number, never
renumbered). An ADR is immutable once accepted: to change a decision, add a new ADR that supersedes it
and set the old one's status to `Superseded by NNNN`.

## Template

```markdown
# NNNN. Title

Status: Proposed | Accepted | Superseded by NNNN

## Context

What forces are at play: the problem, constraints, what was observed.

## Decision

What we do, in one or two paragraphs. Mention the main alternative rejected and why.

## Consequences

What becomes easier, what becomes harder, what must be done as a follow-up.
```

`docs/ARCHITECTURE.md` stays the high-level map; link to the ADR from there when a decision shaped it.
