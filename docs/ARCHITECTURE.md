# RetroDoc architecture (high level)

This page explains **how RetroDoc works** with a few simple diagrams. It is a map, not a spec: for the
roadmap and the rationale behind each choice see [`PLAN.md`](../PLAN.md), for the product vision see
[`PRODUCT.md`](../PRODUCT.md), and for per-module details read the doc comments in the code.

## 1. What RetroDoc does

RetroDoc reads a local Git repo (code, commit history, existing Markdown) and asks an LLM, step by
step, to rebuild the **functional documentation** the project never had: domains → features → use
cases → diagrams, each with a **confidence score** telling how well the code supports the claim.

```mermaid
flowchart LR
    repo[("Local Git repo<br/>code + history + .md")] --> rd{{RetroDoc}}
    cfg["retrodoc.toml"] --> rd
    llm(["LLM<br/>(OpenRouter, or any<br/>compatible server)"]) <--> rd
    rd --> docs["docs/functional/…<br/>Markdown + Mermaid"]
    rd --> rep["docs/_retrodoc/<br/>coverage-report.md"]
    rd --> cache[(".retrodoc/cache/<br/>intermediate artifacts")]
```

## 2. The crates

Six crates; dependencies only point downwards (no cycles).

```mermaid
flowchart TD
    cli["<b>retrodoc-cli</b><br/>commands (clap)"]
    pipe["<b>retrodoc-pipeline</b><br/>LLM passes, caches, report"]
    render["<b>retrodoc-render</b><br/>Markdown/Mermaid files,<br/>diff, idempotent write"]
    ingest["<b>retrodoc-ingest</b><br/>walker, git history,<br/>existing docs"]
    llm["<b>retrodoc-llm</b><br/>LlmProvider trait +<br/>OpenRouter client"]
    core["<b>retrodoc-core</b><br/>domain model + config"]

    cli --> pipe
    cli --> render
    cli --> ingest
    cli --> llm
    pipe --> ingest
    pipe --> llm
    pipe --> core
    render --> core
    ingest --> core
    llm --> core
```

| Crate | Role in one sentence |
|---|---|
| `retrodoc-core` | Shared vocabulary: `Domain`, `Feature`, `UseCase`, `Step`, `ConfidenceScore`, and the `retrodoc.toml` config. |
| `retrodoc-ingest` | Reads the repo: files (honouring `.gitignore`, classified `Source`/`Test`/`Markdown`/`Other`), git history in one pass, existing docs. |
| `retrodoc-llm` | Talks to the model. `LlmProvider` is a trait; the only implementation is `OpenRouterProvider` (HTTP, retry with backoff on 429/5xx). |
| `retrodoc-pipeline` | The brain: one module per pass, plus caches, fingerprints and the debt report. |
| `retrodoc-render` | Turns the result into Markdown files, compares with disk, writes only what changed. |
| `retrodoc-cli` | The `retrodoc` binary: parses arguments and wires the crates together. |

## 3. The commands

```mermaid
flowchart LR
    init["init"] -->|creates| toml["retrodoc.toml"]
    scan["scan"] -->|prints| sum["repo summary<br/>(no LLM, no write)"]
    gen["generate"] -->|LLM calls| art[(".retrodoc/cache/*")]
    gen -->|then| out["docs/"]
    ren["render"] -->|reads, no LLM| art
    ren --> out
    rep["report"] -->|reads, no LLM| art
    rep --> debt["debt report<br/>(stdout)"]
    roles["roles"] -->|LLM, 1 call| rolesyaml["roles.yaml"]
    gloss["glossary"] -->|LLM| glossyaml["glossary.yaml"]
    rolesyaml -.->|required by| gloss
```

- `generate` is the main command; `render` and `report` replay the *last* `generate` without any LLM call.
- `roles` and `glossary` (phase 7) are **standalone for now**: `generate` does not use their output yet.
- "Ask the documentation" (phase 9) is planned in `PLAN.md` but not implemented.

## 4. The `generate` pipeline

This is the core flow. Blue-ish steps call the LLM; the others are deterministic.

```mermaid
flowchart TD
    A["1. Ingestion<br/>files + git history + existing docs"]:::det
    B["2. Repo map<br/>summary per file, then per directory<br/>(bottom-up)"]:::llm
    C["3. Domains<br/>cluster directory summaries into<br/>domains / sub-domains<br/>+ mechanical file expansion<br/>+ coverage repair"]:::llm
    D["4. Features<br/>one call per domain / sub-domain"]:::llm
    E["5. Use cases<br/>one call per feature<br/>(steps + actors, from real code)"]:::llm
    F["6. Diagrams<br/>Mermaid sequenceDiagram<br/>from the steps"]:::det
    G["7. Confidence<br/>one call per use case: is each step<br/>supported by the code it cites?"]:::llm
    H["8. Publish<br/>render → plan (diff) → apply"]:::det

    A --> B --> C --> D --> E --> F --> G --> H

    classDef llm fill:#dbeafe,stroke:#2563eb,color:#000
    classDef det fill:#f3f4f6,stroke:#6b7280,color:#000
```

What each pass hands to the next, and where it is saved:

| # | Pass (module) | Input | Output | Saved in `.retrodoc/cache/` |
|---|---|---|---|---|
| 1 | ingest (`retrodoc-ingest`) | repo path | files, history, existing docs | — (recomputed) |
| 2 | `repo_map.rs` | source files + history | file and directory summaries | `repo-map.json` |
| 3 | `domains.rs` | directory summaries + doc titles | `DomainMap` (every source file in exactly one domain) | `domains.yaml` |
| 4 | `features.rs` | one domain + its files' summaries | `Feature`s grounded on a validated subset of files | `features.yaml` |
| 5 | `use_cases.rs` | one feature + numbered code excerpts | `UseCase`s with steps and actors | `use-cases.yaml` |
| 6 | `diagrams.rs` | use case steps | Mermaid text attached to each use case | `use-cases.yaml` |
| 7 | `confidence.rs` | use case + the code its steps cite | scores on use cases and features | `features.yaml`, `use-cases.yaml` |
| 8 | `report.rs` + `retrodoc-render` | all of the above | Markdown files + debt report | — (written to `docs/`) |

### Safety nets built into the passes

- **Domains coverage is enforced by construction.** If the LLM forgets a file it goes into a synthetic
  "uncategorized" domain; a file assigned twice keeps the first assignment; invented paths are dropped.
  The run is never failed for this, and the repairs are reported (`CoverageReport`).
- **Lenient JSON.** LLM answers are parsed leniently (first JSON value, code fences tolerated). An
  unparseable answer is retried once, then that single unit is skipped with a warning instead of
  aborting the run (`response.rs`).
- **Grounding.** Source references to files outside the feature are dropped; a use case that cites no
  readable code scores 0 without even calling the LLM.

### How the confidence score is computed

```mermaid
flowchart LR
    s["Each step gets a verdict:<br/>supported / partial / unsupported"] --> u["Use case score =<br/>mean of its steps<br/>(steps citing no code capped at 0.25)"]
    u --> f["Feature score =<br/>mean of its use cases<br/>(0 if it has none)"]
    f --> r["Debt report:<br/>per-domain confidence,<br/>sections below 50%"]
```

An unparseable verdict leaves the use case **unscored** (`None`), which the report lists separately.

## 5. Incremental re-run

Re-running `generate` should not pay for the LLM twice. Fingerprints and content hashes decide what to
skip:

```mermaid
flowchart TD
    start(["generate"]) --> force{"--force ?"}
    force -- yes --> wipe["wipe caches"] --> redo
    force -- no --> redo["run passes"]

    redo --> p2{"File content hash<br/>unchanged?"}
    p2 -- yes --> r2["reuse file summary"]
    p2 -- no --> c2["LLM: summarize file"]

    redo --> p3["Domains: always recomputed"]

    redo --> p4{"Domain's files and<br/>summaries unchanged?"}
    p4 -- yes --> r4["reuse features"]
    p4 -- no --> c4["LLM: derive features"]

    redo --> p5{"Feature text and<br/>file contents unchanged?"}
    p5 -- yes --> r5["reuse use cases,<br/>diagrams, confidence"]
    p5 -- no --> c5["LLM: derive use cases"]

    redo --> p7{"Use case already<br/>has a score?"}
    p7 -- yes --> r7["keep score"]
    p7 -- no --> c7["LLM: score"]
```

Directory summaries and `domains.yaml` are cheap enough to always recompute. Note that domain clustering
is not fully deterministic across runs, which can invalidate downstream fingerprints (see `PLAN.md` §7).

## 6. Publishing the docs (no LLM)

`generate` ends with the same code path that the `render` command runs alone.

```mermaid
flowchart LR
    art[(".retrodoc/cache/<br/>features, use-cases,<br/>domains")] --> rpt["build_report"]
    art --> rnd["render()<br/>files in memory,<br/>deterministic"]
    rpt --> rnd
    rnd --> pl["plan()<br/>compare with disk:<br/>created / updated + diff"]
    pl --> dry{"--dry-run ?"}
    dry -- yes --> show["print the diff only"]
    dry -- no --> app["apply()<br/>write only files that differ"]
```

Guarantees:

- **Idempotent**: same artifacts → same bytes → nothing written. The timestamp in
  `_retrodoc/run-metadata.json` is ignored when it is the only difference.
- **Non-destructive**: stale files under `functional/` are reported, never deleted.
- **No feedback loop**: `functional/` and `_retrodoc/` are removed from the "existing docs" read at
  ingestion, so generated docs are never fed back to the LLM.

## 7. What lands on disk

```
<target repo>/
├── retrodoc.toml                  # config (LLM model, ignores, docs dir) — you edit this
├── .retrodoc/cache/               # intermediate artifacts (pipeline state)
│   ├── repo-map.json              #   file summaries, cached by content hash
│   ├── domains.yaml               #   domain clustering
│   ├── features.yaml              #   features (+ confidence)
│   ├── use-cases.yaml             #   use cases, steps, diagrams (+ confidence)
│   ├── fingerprints.json          #   what the incremental re-run compares against
│   ├── roles.yaml                 #   (phase 7) glob → role rules, hand-editable
│   └── glossary.yaml              #   (phase 7) business entities, also its own cache
└── docs/                          # output dir (`output.docs_dir`)
    ├── functional/<domain>/…      #   README per domain, feature pages, use-case pages
    └── _retrodoc/
        ├── coverage-report.md     #   documentation debt report
        └── run-metadata.json      #   model, commit, timestamp
```

## 8. The LLM boundary

All model access goes through one trait, so passes can be tested with a fake and a second provider can
be added later without touching the pipeline.

```mermaid
flowchart LR
    passes["pipeline passes<br/>(repo_map, domains, features,<br/>use_cases, confidence, roles, glossary)"] --> trait["trait LlmProvider"]
    trait --> or["OpenRouterProvider<br/>HTTP + exponential-backoff retry"]
    trait -.-> fake["test fakes<br/>(e.g. CountingProvider)"]
    or --> ep[("OpenRouter, or any server speaking the<br/>OpenAI chat-completions format<br/>via llm.base_url (e.g. local Ollama)")]
```

## 9. Where to go next

| I want to… | Look at |
|---|---|
| Know what is planned / out of scope | `PLAN.md` §5–§7 |
| Add or change a pipeline pass | `crates/retrodoc-pipeline/src/` (follow the existing pass + its fake-provider test) |
| Change the generated Markdown | `crates/retrodoc-render/src/markdown.rs` |
| Add a CLI command | `crates/retrodoc-cli/src/commands/` + `main.rs` |
| Change config options | `crates/retrodoc-core/src/config.rs` |
