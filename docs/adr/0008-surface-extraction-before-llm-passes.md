# 0008. Surface extraction (roles, glossary, entry points) before the LLM passes

Status: Accepted

_Retroactive ADR, reconstructed from the history (b9c9789, 86f3327, 8ffcf06, eb9e1f0, 51276ac, 1dbb8d7; 2026-10-01 to 2026-10-03). Decided 2026-10-01 after the first smoke tests, see `PLAN.md` §7.1._

## Context

The first real runs produced technical documentation: a use case "format children companies" with the
actor "Developer", steps restating method calls, and a top domain named "presentation-layer". The
pipeline climbed from code to business, sending every file to the LLM, which was both costly and
technical. The inputs and outputs of an application and its models are the business, expressed in code.

## Decision

Add a mostly deterministic surface layer between ingestion and the repo map:

- **Roles**: one LLM call over the file tree and manifests (`Cargo.toml`, `Gemfile`, `package.json`…)
  identifies the stack and returns glob rules `pattern -> role`, plus `source_extensions` that promote
  unknown languages to `Source`. Rules are applied mechanically (most specific wins), saved in
  `roles.yaml`, hand-editable, and reused unless `retrodoc roles --force`. No per-ecosystem adapters.
- **Glossary**: entities from `model`-role files (LLM), vocabulary from test descriptions
  (mechanical, tests are `FileKind::Test` and stay out of the rest of the pipeline).
- **Entry points** (routes, commands, jobs, public API) with their outputs, from `entrypoint`-role files.
- **Surface**: a capped prompt section built from the two above; domains are clustered from business
  concepts, never layers. Use cases start from an entry point and receive the code it traverses
  (`CodeIndex`/`slices.rs`, identifier-based, ambiguous names excluded), not the whole feature.

Per-language adapters and a purely bottom-up pipeline were rejected.

## Consequences

The LLM reads only the files that matter, and domains read as business areas. Quality depends on the
LLM recognizing the stack from the tree alone (rules are cached and editable as the safety net; two
runs disagree on borderline folders). Static call tracing in dynamic languages stays heuristic. The
`.erb` views stay `Other` on purpose (about 1,250 files would add as many summary calls).
