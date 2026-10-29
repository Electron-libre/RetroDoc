# 0036. Language and framework agnostic by design, stack knowledge as accelerators

Status: Accepted

## Context

RetroDoc must document a repository whatever its stack. `PLAN.md` §7.1 rejected an adapter per framework: the
roles call recognises the stack from the tree and the manifests (ADR 0008), and a long file of an unknown language
is cut by a boundary regex the LLM proposes (ADR 0013). Knowledge of given stacks has still grown in the code, each
time from the smoke and benchmark repositories: the built-in source extensions of the walker, the test-path
heuristic, the manifests the brief reads, the `rails_schema` reader, the tree-sitter grammars (ADR 0022, five
languages). Nothing states which stacks must work nor that the pipeline must work without that knowledge, so the
tables drift towards the stacks that happen to be measured (Java, C# and PHP test files, Gradle and .NET manifests
are not known today).

## Decision

The generic path is the product: every pass works on a repository of a stack the code knows nothing of, from what
the LLM reads of the tree. A table or reader of a given stack is allowed only as an accelerator (fewer calls,
sharper input) with a generic fallback, and the prompts keep one text for every stack.

A reference set of the most frequent stacks (the most used languages and web frameworks of the Stack Overflow
Developer Survey, reviewed once a year) is named in `specs/language-agnostic.md`: each of its languages is known by
the built-in tables and parsed by tree-sitter, and each of its frameworks has a minimal application that a test runs
through `generate` with a fake LLM.

Rejected: an adapter per framework (a route or ORM parser each, `PLAN.md` §7.1), and supporting only the stacks of
the measured repositories (the current drift).

## Consequences

* A test fixture per framework of the reference set to keep up, and its review when the set changes.
* More tree-sitter grammars: build time and binary size grow, each grammar stays behind its cargo feature.
* A new table or reader of one stack comes with the test of its fallback (`LNG-10`).
* The quality per stack is not required by the spec: it is measured by the quality benchmark.
