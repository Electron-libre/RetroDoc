# RetroDoc v1 Plan

This document details the first version (MVP) of RetroDoc. The long-term scope is described in
[PRODUCT.md](./PRODUCT.md); this plan covers only what is built in v1.

## 1. Scope

| | Decision |
|---|---|
| Scope | Functional documentation only (domains → features → use cases → steps/actors → process diagrams) |
| Input | Local Git repo (code + commit history) + existing Markdown documents supplied as input |
| LLM | OpenRouter, via an abstraction interface (`trait LlmProvider`) so the model choice isn't locked in |
| Output | Markdown + Mermaid files written directly into `docs/` of the analyzed repo |
| Reliability | Each generated section carries a **confidence score**, aggregated into a coverage report |

Out of scope for v1 (kept in PRODUCT.md's long-term vision): technical/C4 docs, GitHub/GitLab/Jira
connectors, automatic PRs, multi-provider LLM.

## 2. Generation pipeline

```
1. Ingestion       → repo walker (respects .gitignore) + parsing of existing .md files + git log per path
2. Repo map        → bottom-up summary per file/module (probable role, enriched with commit frequency/authors)
3. Domains         → LLM clustering of the repo map + existing docs → domains.yaml (intermediate, validated: 100% code coverage, no overlap)
4. Features        → per domain, grounded on the associated code chunks
5. Use cases       → per feature, steps + actors/actions, grounded on real code (handlers, routes, UI, DB)
6. Diagrams        → Mermaid (flowchart/sequenceDiagram) per use case
7. Confidence      → per-section cross-check pass (the LLM checks the claim against the cited code)
8. Writing         → Markdown into docs/, dry-run + diff preview before writing, idempotent on re-run
```

Step 3 (`domains.yaml`) is a structured intermediate artifact, not a final file — this allows steps 4-7 to
be rerun without redoing the clustering, and it's the basis for the incremental cache (rerun only what
touches files modified since the last run).

## 3. Expected output

```
docs/
  functional/
    <domain>/
      README.md                    # domain overview + sub-domains, confidence
      <sub-domain>/
        <feature>.md                # list of use cases
        use-cases/<use-case>.md    # steps, actors, Mermaid diagram, confidence
  _retrodoc/
    coverage-report.md             # identified gaps, low-confidence sections
    run-metadata.json              # model used, date, analyzed commit
```

## 4. Technical architecture (Rust, workspace)

- `retrodoc-cli` — binary, `clap` (`init`, `scan`, `generate`, `report`)
- `retrodoc-core` — domain model (Domain, Feature, UseCase, Step, Actor, ConfidenceScore)
- `retrodoc-ingest` — walker (`ignore`), history (`git2`), Markdown parsing
- `retrodoc-llm` — provider abstraction + OpenRouter implementation (chat completion, retry, rate-limit)
- `retrodoc-pipeline` — multi-pass orchestration + incremental cache (`.retrodoc/cache/`, key = content hash)
- `retrodoc-render` — Markdown/Mermaid writing, diff preview

## 5. Indicative roadmap

1. **Foundation**: CLI skeleton, config (`retrodoc.toml`: OpenRouter key/model, ignored paths), basic ingestion
2. **Repo map**: bottom-up summaries + git history enrichment
3. **Domains/sub-domains**: clustering + coverage validation
4. **Features → use cases → steps/actors** + Mermaid diagrams
5. **Confidence score** + documentation debt report
6. **Writing & CLI ergonomics** (dry-run, idempotence, incremental re-run)

## 6. Identified risks

- Without Jira, domain clustering risks reflecting the code's *technical* structure rather than the real
  *business* breakdown — the confidence score must explicitly distinguish the two.
- Token cost/volume on a large repo: the incremental cache (pipeline step) matters from v1, not a
  "nice to have" for later.
- The default OpenRouter model choice (quality vs. cost) is still to be settled at implementation time.
