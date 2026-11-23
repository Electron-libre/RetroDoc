# 0055. Slices follow the references resolved per language, the lexical follow as fallback

Status: Accepted

## Context

A slice is the code a use case is derived from: the files its entry point reaches in two hops, eight at most.
It followed the identifiers a file writes to the files named like them (`slices.rs`). Following the calls the
`tags.scm` queries give, resolved by bare name, brought as much noise as it removed: a method called on a
receiver of unknown type went to any file defining that name (ADR 0027,
`issues/done/parsed_references_for_ranking_and_slices.md`).

codegraph, which resolves through imports, parent classes and receivers, removed that noise where it linked, but
linked nothing for a fifth of the entry-point files (strings such as `call_command`, `.rake` tasks, scripts)
(`docs/findings.md` §13). Its edges first and the lexical follow for the files they leave empty beat the
lexical slice on a sample read with no LLM; depending on codegraph was rejected (its own Node runtime, telemetry,
an internal schema) for a port into RetroDoc (`docs/findings.md` §14).

## Decision

`retrodoc-ingest` resolves the references of the source files itself (`resolve/`, `specs/resolved-references.md`):
one resolver per language behind its grammar feature, written as its own tree-sitter walk, for Python and Ruby
first. A reference resolves through the language's rules only: imports and requires, constants by namespace,
calls on `self`, with no receiver or on a known class, looked up in the class and its ancestors; a call on any
other receiver resolves to nothing, never to a file guessed from the method's name. No framework convention is
read (ADR 0036).

`CodeIndex::slice`, which every pass uses, follows the resolved references from a file that has some and the
lexical follow from any other file (a language with no resolver, a file the build cannot parse, a file linked by
strings only), with the same depth, budget and ranking.

The main alternative, calling codegraph when installed with today's path as fallback, was rejected: two paths to
keep measured, and a runtime and telemetry outside the single binary.

## Consequences

* Slices of Python and Ruby files hold the models, collaborators and parent classes the code uses, without the
  files named like a method or a word; the files a job or service reaches only through a parameter's name (no type
  inference) are lost.
* A language gets the gain once its resolver is written; until then it keeps the lexical slices.
* The saved use cases are derived again once where a slice changes (the files of a slice are in their
  fingerprint).
* `ranking.rs` and the placement of contexts and routes still count names lexically.
