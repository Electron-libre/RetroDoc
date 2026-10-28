# 0027. Cut long files at parsed definitions, and offer short method bodies as an option

Status: Accepted

## Context

ADR 0022 chose tree-sitter to replace the LLM-negotiated chunk regexes and to give the passes a
deterministic view of the code. Its first deliverables were built and measured on the local model
(`issues/done/tree_sitter_structure.md`):

* On a small Ruby gem the regex was checked at 100% and cost nothing; on a Rails repository the Ruby
  regex scored 98% and the JavaScript/TypeScript one 0% at the first try, which meant one warning and
  one more LLM call (ADR 0013).
* The five grammars (Ruby, Rust, JavaScript, TypeScript, Python) build in about 9 s in release and add
  7.4 MB (+35%) to the release binary.
* 51% to 83% of the characters of the files the entry points and glossary passes read sit in method or
  function bodies.

## Decision

* `retrodoc_ingest::structure` parses a file with the grammar's own `tags.scm` query and returns its
  definitions (kind, line range, signature) and the names it calls. One cargo feature per language, all on
  by default. Any other file gets `None` and keeps the old paths.
* `Splitter` cuts a parsed file before its definitions, moved up over the comment, attribute and
  decorator lines above each one. A parsed file without definitions, and every other language, keeps the
  regex rule, then blank lines. The roles call is told not to propose regexes for the parsed extensions and
  `chunk_check` drops them without a call; a saved `roles.yaml` stays valid.
* `[pipeline] body_lines = N` is an option, off by default: the glossary and entry points passes read the
  signature and the first N lines of each outermost method or function body; the use cases and
  confidence excerpts are untouched. It is part of those passes' file hash.

The option is not the default because the measure does not clear it. On linkding, three runs per setting
with the local model: prompts 42% smaller at N=5 (52% at N=2), the entry points found within the run-to-run
noise (groups of the reference 79-93% in every setting), but the number of attributes in the glossary lower
at N=5 in all three runs (77-94 against 103-132) and no reference to say whether they matter.

Not done: references from the parse for `ranking.rs` and `slices.rs`. Measured with no LLM
(`issues/parsed_references_for_ranking_and_slices.md`, example `parsed_slices`): slices that follow the parsed
calls (resolved to the definitions of the same language, lexical follow for the rest) change about 40% of the
files of a Rails slice, but bring in as much noise as they remove, as a method called on a receiver of unknown
type resolves to any file defining that name; resolved to classes, modules and functions only, they are almost
the lexical slices. The `tags.scm` queries give calls (and, for Ruby, the constants read), no types or imports.
The lexical slices and ranking stay.

## Consequences

* No regex to negotiate, check or fix for five languages: fewer calls and warnings at the roles step, and
  `chunk_check` only runs for the others.
* A bigger binary and a longer build; a language is dropped by turning its feature off.
* Two chunking paths remain for the languages without a grammar here.
* `body_lines` can be tried per repository; making it the default needs a reference for the glossary and
  more runs than the noise allows today.

## Follow-up (2026-10-27)

`[pipeline] body_lines` is removed (`issues/done/settle_experimental_settings.md`). Measured again with the joint
reading of ADR 0028 on linkding (`deepseek-chat`, three runs per setting): N=5 cuts the input of the reading
passes by 38%, which is about 4% of a run with the rules on; the glossary loses only the form classes, the
model entities keep their attributes; the entry points stay within the spread but the only move is down (one run
finds one group fewer, 73% of its entries in reference files against 88-92%). The files are read whole; a
`retrodoc.toml` that still names the key is refused with a message saying so (`specs/pipeline-settings.md`, SET-4).
