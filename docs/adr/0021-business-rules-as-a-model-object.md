# 0021. Make business rules an object of the model

Status: Accepted

## Context

The model is `Domain → Feature → UseCase → Step` (`retrodoc-core/src/model.rs`). It has no place for the
rules of the business: who may do what, which states an object can move through, which values are
refused, which limits apply. Those rules are what a functional reader most often asks about ("can a
signed contract be edited?"), and today they only appear by chance inside a step or a narrative.

They are often easy to locate: validations and constraints of the models and the schema, authorization
policies and abilities, state machines and enums, guard clauses that raise a domain error, constants
with business names, and test descriptions that state a behaviour ("refuses the signature when the
contract has expired"). The test descriptions are already extracted mechanically and unused (ADR 0019).

## Decision

Add a `BusinessRule` to the model: a statement in business language, its kind (validation, permission,
state transition, limit, computation), the entities and use cases it applies to, its evidence (code
lines, test phrases, doc sections) and a confidence.

A rules pass finds candidate constructs mechanically (with the structure of ADR 0022 where available,
lexical patterns otherwise) in the business files and the slices of the use cases, then asks the LLM to
state each rule in business terms, in batches. A rule backed by a test phrase or a doc section as well as
by code gets a higher confidence than one backed by code alone.

Rules are rendered on the entity (glossary) and use case pages, listed per domain, and served by the MCP
server.

Rejected: leaving rules inside the use case narratives (not searchable, not countable, not checkable
against the docs).

## Consequences

* A new pass with its cache and fingerprint, bounded by the business surface (ADR 0020).
* The debt report can count the rules that no doc mentions.
* The candidate detection is language-dependent; it starts with the languages of the smoke repositories
  and degrades to the LLM reading the slice.
* Follow-up issue: `issues/business_rules.md`.
