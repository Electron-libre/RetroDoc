# RetroDoc

## Goal

Catch up on a software project's documentation debt by putting AI agents to work.

## v1 scope (MVP)

v1 is limited to **functional documentation**, starting from a **local Git repo** (code + history +
existing Markdown documents), with **OpenRouter** as the LLM provider. The detailed plan and architecture
are in [PLAN.md](./PLAN.md).

The rest of this document describes the long-term product vision; the sections not covered by v1
(technical docs, GitHub/GitLab/Jira connectors, automatic PR, multi-provider) are future targets.

## Features

* Identify missing documentation
* Write missing technical documentation
* Write missing functional documentation
* Answer questions about how the application works, from the documentation (generated and collected),
  the change history and, when needed, the code


## Inputs

* Source code
* Change history
* Tickets
* Existing documents

## Outputs

Markdown + Mermaid or SVG format

* Technical documentation
* Functional documentation
* C4 diagrams
* Answers to questions about the application, with cited sources (CLI `ask` / `chat`)

## Prerequisites

* Connectors to GitHub, GitLab, Jira
* OpenAI connectors for model selection

## Process

### Functional doc

* Identify the application's domains and sub-domains
* For each domain, identify the features
* For each feature, identify the use cases
* For each use case, identify the steps
* For each step, identify the actors and actions
* For each process, create a process diagram and document it.

### Technical doc

* For each component, create a component diagram and document it.
* Link the component to its domain, sub-domain, and use case.
* Document the deployment process.
* Document the APIs.

### Ask the documentation

Once the documentation is built, the most useful way to consume it is to question it: an agent answers
questions about how the application works ("what happens when a contract is signed?", "who can cancel a
subscription?").

* Navigate the structure (domains, features, use cases) to find the relevant zone, instead of searching a
  flat pile of text.
* Rely on the generated documentation, the collected documents and the change history; go down to the
  code only when the question is technical or the documentation is uncertain.
* Cite the features, use cases and files behind every answer, so it can be checked.
* Use the confidence scores: flag answers that rest on low-confidence sections, and warn when the code has
  changed since the documentation was generated.
* Admit when something is not documented, and feed those gaps back into the documentation debt report.

#### For LLM agents (MCP)

The documentation is also meant to be read by other agents, such as coding agents. RetroDoc exposes it
through a local MCP server: read-only tools to list domains, open a feature or use case, and search the
documentation (lexical search, no embeddings unless measured as necessary). The server needs no LLM of its
own, since the calling agent does the reasoning. A static index for agents (`llms.txt`-style or an
`AGENTS.md` section) is generated next to the docs for tools that don't run the server.

## Technologies

* CLI
* Rust
