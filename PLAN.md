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
3. Domains         → LLM clustering of the repo map's directory-level summaries + existing docs (files resolved to domains by longest-prefix path match) → domains.yaml (intermediate, validated: 100% code coverage, no overlap)
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
        use-cases/<feature>/<use-case>.md    # steps, actors, Mermaid diagram, confidence
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
7. **Business-level documentation** (see §7.1): the output must read as functional/business documentation,
   not as a paraphrase of the code. Not started.
8. **Scalability & cost control** (see §7.2): bring a run on a large repo (thousands of files) within
   reach, including on limited/local LLM resources. Not started.

## 6. Identified risks

- Without Jira, domain clustering risks reflecting the code's *technical* structure rather than the real
  *business* breakdown — the confidence score must explicitly distinguish the two.
- Token cost/volume on a large repo: the incremental cache (pipeline step) matters from v1, not a
  "nice to have" for later.
- The default OpenRouter model choice (quality vs. cost) is still to be settled at implementation time.
- **Both risks above materialized in the first real smoke tests** (§7): the generated docs are mostly
  technical and the run time does not scale. They are now the priority work, ahead of new features
  (phases 7 and 8).

## 7. Smoke-test findings and next work (2026-10-01)

Smoke tests were run with `qwen3.6:35b-a3b` on a local Ollama (AMD iGPU, 32k context,
`reasoning_effort = "none"`), on throwaway `git worktree`s in `/tmp` (see CLAUDE.md for the CLI commands):

| Repo | Size | Full run (`--force`) | Incremental re-run | Result |
|---|---|---|---|---|
| autoroute (Rust, actix macros) | 6 source files | ~8 min | ~75 s | 6 features, 17 use cases |
| Rails test repo / `app/presenters/` (Rails) | 26 files | ~31 min | ~53 s | 21 features, 72 use cases, 91% confidence |

Already fixed (same session, commit `5543f28` and following):
- **Unstable domains broke the incremental re-run.** The clustering is non-deterministic, domain slugs
  changed on each run, and every downstream fingerprint is keyed by slug, so a "no change" re-run redid
  everything (8 min). `build_domains` now fingerprints its input (file paths + summaries + existing docs;
  directory summaries excluded, they are regenerated each run) and reuses `domains.yaml` when unchanged.
- **Cited paths rejected → use cases at 0%.** The LLM shortens `crate/src/a.rs` to `src/a.rs`; the strict
  match dropped the reference and the confidence pass scored the use case 0 ("no readable code cited").
  `resolve_cited_path` now accepts a unique suffix match.
- **Empty LLM answer silently accepted.** A feature could end up with no use case and no warning; an
  answer with zero use cases is now logged and retried once.

### 7.1 Phase 7 — business-level documentation (open)

Observed: the documentation is mostly technical and the business domain is barely perceptible. Example
(the Rails test repo): a use case "Format children companies for UI selection" has actor "Developer", five steps that
restate method calls (`children_companies`, `map`), and a sequence diagram between a developer and a Ruby
object. The top domain is named "presentation-layer" — a technical split, the very risk listed in §6.

Causes identified in the current design:
- Prompts ask to describe "what is visible in the code" and forbid inventing, which yields function-level
  steps. Actors are limited to `human`/`system`, hence "Developer" / "CompanyPresenter" rather than business
  roles (contract manager, signatory, partner…).
- The unit of work is a file/class; the domain clustering only sees technically-worded directory summaries
  (no models, routes, tests, glossary).
- The confidence pass rewards closeness to the code, which favors literal paraphrase over business meaning.

Directions, most valuable first:
1. Two output levels: a business narrative (who does what, why, with which vocabulary), still grounded on
   files but not on a line-by-line paraphrase; the technical detail (steps, code refs) kept below/folded.
2. Richer inputs: model/entity names, routes, migrations and **tests** (they often name business
   behaviours), a glossary extracted from existing docs and from commit messages/ticket references.
3. Business actors proposed at the domain stage and imposed on use cases (instead of free-form
   `human`/`system`).
4. A check on the vocabulary itself ("is this business language?") as a criterion next to confidence, and
   domain naming that avoids layer names (presentation, infrastructure, utils).
5. Caveat: part of the quality is bounded by the local model. Re-run the same prompts with a stronger model
   to tell prompt problems from model limits before tuning prompts.

Validation: re-run on the Rails test repo and autoroute; compare actors, domain names and use-case titles by hand
(a reader who doesn't know the code should be able to say what the product does).

### 7.2 Phase 8 — scalability & cost control (open)

Observed: the cost is linear in LLM calls — one per file, per directory, per domain unit, per feature, per
use case, plus one **per use case** for confidence (about a third of all calls). 26 files already took 31
min locally; the ~2,300-file Rails test repo is out of reach in this form.

Directions:
1. Bound the depth by default: spend LLM calls on what matters (most-changed, most central, most
   business-relevant files), roll the rest up at directory level.
2. Batch calls: several small files per summary request, several use cases per confidence request.
3. Make the confidence pass optional or sampled (`--no-confidence`, `--confidence-sample`).
4. Configurable concurrency (little gain on a single local model, large on OpenRouter).
5. Print an estimated call/token count before running, and make an interrupted run resumable (the file
   summaries are already cached; features/use cases are only saved at the end of each pass).
6. Directory summaries are still regenerated on every run (cheap on small repos, not on large ones):
   cache them by the hash of their children's summaries.

Validation: measure calls and wall time before/after on the Rails test repo `app/presenters/` (baseline above), then
on a larger sparse-checkout (e.g. `app/models` + `app/services`).

### 7.3 Smaller open items

- `.erb` view templates are not classified as `Source` by the walker (1,253 files on the Rails test repo).
- Retry on an empty/invalid LLM answer fixes symptoms; the underlying causes (context length, truncated
  JSON) depend on the server config — see the local LLM setup notes.
- Truncation: `MAX_CHARS_PER_FILE` (4,000) silently cuts long files, so late code can never be cited.
