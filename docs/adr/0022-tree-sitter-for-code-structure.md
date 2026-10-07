# 0022. Read the code structure with tree-sitter, keep the LLM for meaning

Status: Accepted (supersedes 0013 for the languages it covers)

## Context

The pipeline has no parser. Where to cut a long file is negotiated with the LLM: the roles call proposes
a boundary regex per language, `chunk_check.rs` measures it on the largest files and asks for fixes (up
to three calls), and drops it below 50% (ADR 0013). This works but is a source of warnings
(`issues/use_cases_model_noise_warnings.md`) and of code (`chunks.rs` and `chunk_check.rs`, about 900
lines). Symbols and references are guessed lexically (`ranking.rs` counts the files mentioning a file's
name, `slices.rs` follows referenced identifiers). The entry points and glossary passes send whole file
chunks where signatures would often be enough.

`PLAN.md` §7.1 rejected per-ecosystem adapters (one per framework). Tree-sitter grammars are per language,
not per framework, and most of them ship a `tags.scm` query that lists definitions and references.

## Decision

Use tree-sitter, for a first set of languages (those of the smoke repositories, then the most common
ones), to get:

* exact definition boundaries for chunking, replacing the LLM regexes for those languages;
* the symbols of a file (classes, methods, functions, with their signatures) and the references between
  files, which feed the deterministic map and ranking (ADR 0020) and the slices;
* signature-only views of a file, sent instead of the full text where the pass needs names, not bodies
  (entry points of a controller, entities of a model).

Languages without a grammar keep the current path (LLM regex, then blank lines).

Rejected: a language server per language (heavy to install and run on a user's machine), and staying
lexical (the cost is the warnings and the extra calls above).

## Consequences

* Fewer calls and fewer warnings at the chunking stage, smaller prompts for the extraction passes.
* Build time and binary size grow with each grammar; the grammars should be behind cargo features.
* Two code paths for chunking until most languages are covered.
* Follow-up issue: `issues/tree_sitter_structure.md`.
