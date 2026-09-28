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

## Technologies

* CLI
* Rust
