---
priority: medium
why: 'the docs are written in the language the model picks (English on the saved runs), so a team that reads another language cannot take them as they are; it touches every writing pass'
---
# Let the user choose the language of the generated documentation

# Goal

A `retrodoc.toml` key sets the language of the generated docs (names of the domains, features and use cases,
narratives, steps, rules, glossary definitions, brief), for example French. Without it the behaviour stays that of
today. The fixed text of the render and of the web interface (headings, labels) follows the same language, or the
issue says it does not.

# Findings

* `crates/retrodoc-core/src/config.rs` has no language key; no prompt of `crates/retrodoc-pipeline` names the output
  language (the brief asks for "business language", the register, not the tongue).
* The saved runs of public apps are written in English (healthchecks, linkding, lobsters); what a model writes for a
  repository whose comments and commits are in another language has not been measured.
* The render and the web interface have their fixed text in English (`<html lang="en">`, "Search the docs").

# Approach

1. Measure first: the language of the docs of a saved run whose code or commits are not in English (the large Rails
   app), to know whether the model already drifts.
2. An `[output] language` key (a name the model reads, as "French", or a BCP 47 tag mapped to one), added to every
   writing prompt as one instruction; the slugs and file names stay derived from the names, so check that the
   slugs of accented names stay stable.
3. The language is part of the input hash of each writing pass: changing it reruns them, keeping it is a no-op.
4. The scores that judge the docs (business language, confidence, the benchmark against an English reference) must
   still work on docs in another language: measure one pass at least.
5. The fixed text of the render and the interface: translated tables for a few languages, or left in English and said
   in the non-goals.

# Specs

* The config spec and the specs of the writing passes amended; `specs/web-ui.md` if the interface follows.

# Resources

* `crates/retrodoc-core/src/config.rs`, the prompts of `crates/retrodoc-pipeline/src/` (one module per pass).
* `docs/crates/pipeline.md` (input hashes, incremental re-run), `docs/crates/render-mcp-cli.md`.
* The measures ledger before any measure (paid runs: ask first).

# Hints

* `bm25.rs` already strips English and French stop words: the search keeps working on French docs.
* The glossary matches the names of the code (English identifiers) to the concepts: a translated concept name must
  keep its link to the identifier.
