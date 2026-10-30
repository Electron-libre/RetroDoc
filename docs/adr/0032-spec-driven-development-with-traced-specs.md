# 0032. Spec-driven development with traced specs

Status: Accepted

## Context

The expected behaviour of a capability was spread over `PLAN.md` (roadmap, since replaced by `ROADMAP.md`), the ADRs (why a choice was made),
the `//!` doc of each pass and the issues, which are short-lived. Nothing stated, per capability, what must hold
for it to be correct, and nothing tied such a statement to the test that proves it, so the documentation of
behaviour could drift from the tests unnoticed.

## Decision

Each capability has a spec in a root `specs/` directory: numbered requirements with stable ids (`RER-1`),
Given/When/Then scenarios for the edge cases, non-goals, links to the ADRs, a status (`draft`, `accepted`,
`implemented`). A requirement is `deterministic` (proved by a test with a fake `LlmProvider`) or `quality`
(proved by the benchmark, since a model's output cannot be asserted). A test cites the requirements it proves
with a `// spec: ID` comment, and a `rust-script` check, part of `just check`, fails when a spec and its tests
drift apart. A change of behaviour starts with the spec, validated by the user before the plan and the tests.
Specs are written capability by capability, when a capability is next changed, not all at once.

Rejected: a comment-free convention (test names carrying the id), which fits badly the descriptive names the
tests have; and a benchmark run inside the check, too costly and too noisy (15 to 20 points between runs) to
block a commit.

## Consequences

`rust-script` becomes a requirement of `just check`, not only of `just test-harness`. A spec is one more file
to keep true; the check keeps it honest for `deterministic` requirements only, a `quality` one is as good as
its benchmark. The ids are an interface: they stay once a spec is accepted.
