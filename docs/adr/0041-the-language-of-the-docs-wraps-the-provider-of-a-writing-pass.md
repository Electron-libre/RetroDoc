# 0041. The language of the docs wraps the provider of a writing pass

Status: Accepted

## Context

The generated docs are written in the language the model picks: English on every saved run, the large private
Rails app included, whose commit messages are mostly English. A team that reads another language wants them in
it (`issues/done/generated_docs_language.md`). Eleven passes write text a reader sees (brief, contexts, glossary, entry
points, actors, domains, features, use cases, rules, confidence, reconcile); their requests are constants of the
binary (ADR 0040) and their prints hold the model of the pass (ADR 0023), except the brief, whose print hashes its
input alone, and the contexts map, which has no print: it is reused until `contexts --force`.

## Decision

`[output] language` names the language. When it is set, `PassProviders` wraps the provider of each writing pass
in a provider that appends one instruction to the system prompt of every request, naming the language and asking
to keep code identifiers, file paths and given names as they are, and whose `model()` is that of the provider it
wraps followed by the language. The passes that print the model ask their units again when the language changes,
with no change to their code; the brief adds the language to its print when it is set (`LlmProvider::language`).
The contexts map keeps its rule: a new map is written in the language, a saved one stays until `--force`. The
reading passes (sources, roles, business files) and the benchmark judge keep the plain provider: their answers are
not shown and their prints stay valid.

Without the key nothing is wrapped: the requests and the prints are those of today, and no saved run is asked
again.

Rejected: a language field in each `Request` passed down through every pass (eleven call chains changed for one
instruction); detecting the language of the repository (the saved runs show the model already writes English on a
repository whose people write French, and the reader, not the code, decides).

## Consequences

- The model, not the passes, carries the language into the prints: a pass that stops printing its model loses the
  re-run on a language change, and the test of DLG-4 says so.
- The names the passes match across each other (domain names, entity names) are written by the model in the
  language; the instruction keeps the names a prompt gives, so a later pass reuses the spelling of the earlier one.
- Slugs keep the base letter of an accented Latin letter, so a French name gives a readable folder.
