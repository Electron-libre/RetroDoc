# RetroDoc — product

RetroDoc catches up on a software project's documentation debt with LLM agents: it reads what a project already
has (code, commit history, existing docs) and writes the documentation nobody wrote, saying how far each claim can
be trusted.

This document says what the product is for and for whom. The planned work and its state are in
[ROADMAP.md](./ROADMAP.md), how it works in [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md), the decisions in
[docs/adr/](./docs/adr/), the dated findings in [docs/findings.md](./docs/findings.md).

## Problem

Most applications that have lived a few years have no up-to-date description of what they do. The knowledge sits in
the code, in commit messages, in tickets and in a few stale documents, and in the heads of the people who wrote it.
Whoever takes the application over (a new developer, a new team, a product owner, an auditor) rebuilds it by reading
the code, which is slow and yields a technical picture, not a business one. Writing the documentation by hand is the
debt nobody has time to pay.

## Users

| User | What they need from RetroDoc |
|---|---|
| Developer or team taking over an application | Know what the application does, by domain and use case, before reading the code |
| Tech lead or maintainer | See the documentation debt: what is undocumented, what the existing docs contradict, what is low-confidence |
| Product owner, business analyst | A functional description in business language: actors, use cases, business rules |
| Coding agent (through MCP) | Read the functional docs and the code they cite, to work on the application with its business context |

## Principles

- **Business first**: the docs describe what users do and why, in the application's vocabulary, not a paraphrase of
  the code; the technical steps are kept, folded, below the narrative.
- **Grounded and checkable**: every use case cites the code it comes from; every section carries a confidence score;
  what the evidence does not support is reported, not invented.
- **Any stack**: the generic path works on a stack RetroDoc knows nothing of; knowledge of a given stack only makes it
  faster or sharper (ADR 0036).
- **Cost follows the business surface**: the work grows with the entry points and entities, not with the file count
  (ADR 0020), so a large application is in reach, one bounded context at a time if needed.
- **Safe to rerun**: an unchanged repository costs no LLM call and changes no file; an interrupted run resumes.
- **Read-only on the analysed repository**: RetroDoc writes only its docs and its cache; the consumers of the docs
  (MCP server, search) call no LLM of their own.
- **Model choice is the user's**: one provider interface, any OpenAI-compatible endpoint, hosted or local, a model per
  pass if wanted.

## Capabilities

| Capability | v1 | Later |
|---|---|---|
| Functional documentation: domains → features → use cases (narrative, steps, actors) → business rules → diagrams | yes | |
| Confidence score per section and documentation debt report (gaps, low confidence, undocumented behaviours) | yes | |
| Reconciliation with the existing docs: covered, contradicted or undocumented items | yes | |
| Incremental rerun, resume, cost recap per pass | yes | pre-run cost estimate |
| Large applications documented one bounded context at a time | yes | |
| Read-only MCP server and lexical search over the generated docs, for agents | yes | |
| Ask the documentation from RetroDoc itself (`ask` / `chat`) | | yes |
| Technical documentation: components, C4 diagrams, deployment, APIs, linked to the domains and use cases | | yes |
| Inputs from GitHub, GitLab and Jira (issues, tickets, pull requests) | | yes |
| Automatic pull request with the generated docs | | yes |

## User journeys

1. **Document an application**: `retrodoc init` then `retrodoc generate` on a local clone; read
   `docs/functional/` (domain pages, then features, then use cases) and `docs/_retrodoc/coverage-report.md`.
2. **Keep it current**: rerun `generate` after changes; only the units whose inputs changed are redone, and
   `--dry-run` shows the diff before anything is written.
3. **Document a large application**: `retrodoc contexts` maps its bounded contexts; each `generate --context …` run
   documents the next ones and keeps the others.
4. **Pay the debt**: the coverage report lists what the existing docs miss or contradict, and the low-confidence
   sections to review by hand.
5. **Give the docs to an agent**: `retrodoc mcp` serves the domains, features, use cases and the code they cite; the
   answers warn when a cited file changed since the docs were generated.

## Ask the documentation (vision)

Once the documentation is built, the most useful way to consume it is to question it: an agent answers questions
about how the application works ("what happens when a contract is signed?", "who can cancel a subscription?").

- Navigate the structure (domains, features, use cases) to find the relevant zone, instead of searching a flat pile
  of text.
- Rely on the generated documentation, the collected documents and the change history; go down to the code only when
  the question is technical or the documentation is uncertain.
- Cite the features, use cases and files behind every answer, so it can be checked.
- Use the confidence scores: flag answers that rest on low-confidence sections, and warn when the code has changed
  since the documentation was generated.
- Admit when something is not documented, and feed those gaps back into the documentation debt report.

The first form is the MCP server: the calling agent does the reasoning. A static index for agents (`llms.txt`) is
generated next to the docs for tools that do not run the server.

## Scope of v1

In: functional documentation, from a local Git repository (code, commit history, existing Markdown docs), with
OpenRouter or the DeepSeek API (or any compatible endpoint, such as a local server) as the LLM, written as Markdown
and Mermaid into `docs/` of the analysed repository.

Out of v1 (see "Later" above): technical and C4 documentation, GitHub/GitLab/Jira connectors, automatic pull
requests, several provider wire formats.

Non-goals: generating or changing code; a hosted service (RetroDoc is a local CLI); a general retrieval layer
(embeddings) inside the generation pipeline, which must stay deterministic on rerun.

## Success measures

Measured by `retrodoc benchmark` against hand-written references on public repositories (`skill:quality-benchmark`):

- recall and precision of the domains and features against the reference;
- share of the use case narratives in business language;
- calls, tokens and time per run, for a given size of business surface;
- a rerun on an unchanged repository makes no LLM call and changes no file.

## Constraints and assumptions

- A Rust CLI, run locally on a clone; no data leaves the machine except the prompts sent to the chosen LLM endpoint.
- The LLM recognises the stack and its conventions from the file tree and the manifests; a stack table only helps.
- The quality is bounded by the model: a small local model is a valid target, at a lower quality and a longer run.

## Risks

- Without tickets or product docs, the domains may follow the code's technical structure rather than the business
  breakdown; the confidence and business-language scores must tell the two apart.
- Token cost and run time on a large repository; the incremental cache, the bounded contexts and the cost recap are
  there from v1.
- With behaviours as the coverage unit, a behaviour the entry point pass misses is a use case missing from the docs;
  the debt report lists the files no entry point reaches.
- The measures are noisy: the spread between runs is as large as many effects, the LLM judge of the benchmark is
  wrong on a share of its pairs, and the references are not all checked by a human.

## Open questions

- The default model (quality against cost) for hosted and local runs.
- Whether the lexical search is enough on business vocabulary that differs from the code's, or embeddings are needed
  (to measure, not assume).
- For `ask` / `chat`: tool calling in the provider interface, or a JSON-action protocol that local models follow.
