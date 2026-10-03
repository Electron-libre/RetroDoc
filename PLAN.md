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
        use-cases/<feature>/<use-case>.md    # business narrative, then (folded) steps, actors, Mermaid diagram, confidence
  _retrodoc/
    coverage-report.md             # identified gaps, low-confidence sections
    run-metadata.json              # model used, date, analyzed commit
```

## 4. Technical architecture (Rust, workspace)

- `retrodoc-cli` — binary, `clap` (`init`, `scan`, `generate`, `render`, `report`, `roles`, `glossary`, `entry-points`, `actors`, `surface`)
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
   not as a paraphrase of the code. Built on a new "surface extraction" foundation (file roles, models and
   glossary, entry points and outputs) that also bounds what is sent to the LLM. Implemented end to end in
   code (steps 1–4e: surface, actors, narrative, business-language score), each step checked on the real Rails test repo
   data piece by piece; a full `generate` run on a real repo is still missing (blocked on phase 8: the repo
   map of a large repo is out of reach locally).
8. **Scalability & cost control** (see §7.2): bring a run on a large repo (thousands of files) within
   reach, including on limited/local LLM resources. In progress: delivered are progress reporting, the LLM
   heartbeat, the directory-summary cache (item 6), `--no-confidence`/`--confidence-sample` (item 3) and item 5
   (call estimate for the repo map pass, resumable features/use cases); item 4 (`llm.concurrency`, repo map only) and item 2 (`llm.batch_chars`, batched confidence); item 1 (`--max-files`, opt-in) — all six directions are now delivered.
9. **Ask the documentation** (see §7.3): a question-answering agent (`retrodoc ask` / `chat`) grounded on
   the generated artifacts, the collected docs, the git history and, when needed, the code. Not started;
   comes after phases 7 and 8, since answer quality is bounded by the quality of the generated docs.

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

Direction (decided 2026-10-01): sending every file to the LLM is brutal and starts from the wrong end —
it climbs from code to business. The inputs and outputs of the application and its models *are* the
business, expressed in code. Phase 7 therefore starts with a mostly deterministic **surface extraction**
pass, placed between ingestion and the repo map, and rewires the LLM passes on top of it.

Surface extraction:
1. **Technology and role identification by the LLM, from the file tree.** No per-ecosystem adapters: one
   LLM call over the tree (paths, plus manifest files such as `Cargo.toml`/`Gemfile`/`package.json`) first
   identifies the stack and conventions, then says where the entry points, models/entities, business logic,
   views, infra, config and tests live, as path patterns (globs) with a role each. Those rules are applied
   mechanically to every file (longest match wins; unmatched files go to a second pass over just those
   paths, or to "unclassified"). Output: a role per file, the rules (cached, reviewable, hand-editable in
   `.retrodoc/cache/`), and a distribution report. Names and signatures are only sent for ambiguous cases.
2. **Models and glossary.** Entity names, attributes and associations give the nouns; public methods and
   route actions give the verbs. Existing docs and commit messages complete the glossary. **Tests** are a
   third vocabulary source (their descriptions often name business behaviours).
3. **Entry points** (HTTP routes, CLI commands, jobs/cron, message consumers, webhooks) — each is a
   use-case candidate, with a verb and a resource — and **outputs** (responses, emails, generated files,
   emitted events, external API calls, DB writes), the observable effects of a use case.
4. Extraction of models, entry points and outputs is also LLM-driven, but scoped by the role rules: the
   LLM reads only the files classified as model/entrypoint, never the whole repo. A library crate
   (autoroute) has no application I/O: its public API is the entry point, so the surface model must allow
   several kinds of surface.

Rewiring the existing passes on the surface:
5. **Domains** are clustered from models and vocabulary, not from directory summaries; layer names
   (presentation, infrastructure, utils) are forbidden as domain names.
6. **Use cases** start from an entry point; the LLM receives the slice of code it traverses (controller →
   service → model, found by referenced identifiers), not the whole feature. **Actors** are business roles
   proposed at the domain stage and derived from authentication/authorization (roles, policies) when
   present, instead of free-form `human`/`system`.
7. Files outside every slice are summarized cheaply or only listed in the debt report.
8. **Two output levels**: a business narrative (who does what, why, with which vocabulary), grounded on
   files but not a line-by-line paraphrase; technical steps and code refs kept below/folded.
9. A **business-vocabulary criterion** ("is this business language?") next to confidence, so the
   confidence pass stops favouring literal paraphrase. Done last, once there is content to measure.

Caveats: the role rules depend on the LLM recognizing the stack from the tree alone (hence the manifest
files, and the cached/editable rules as a safety net). The vocabulary is a hint to validate, not a truth (fat Rails models vs. logic in services; a
method name can be technical, e.g. `format_for_ui`). Static call tracing is hard in dynamic languages: a
heuristic on referenced identifiers is the starting point. Part of the quality is bounded by the local
model: re-run the same prompts with a stronger model (OpenRouter) to tell prompt problems from model
limits before tuning prompts.

Steps, each shippable and checkable on the Rails test repo:
1. Technology identification and file role rules (one LLM call on the tree), applied mechanically, with the
   distribution report. Shows how many files are relevant before any per-file LLM call.
   **Implemented** (`roles.rs`, `retrodoc roles [--force]`, rules in `.retrodoc/cache/roles.yaml`).
   Smoke test on the full Rails test repo (4,181 files, `qwen3.6:35b-a3b`, ~1m30 per call): ~92% of files
   get a role (~350 unclassified: dotfiles, `Rakefile`, coffee scripts), but two runs disagree on
   borderline folders (mailers/jobs entrypoint vs infra, presenters logic vs view) and one run out of five
   returned unparseable JSON twice — hence the saved, editable rules. autoroute not tried yet. Not done: the second pass over unmatched files (they stay `unclassified`) and
   wiring the roles into `generate`.
2. Models and glossary inventory. **Implemented** (`glossary.rs`, `retrodoc glossary`, needs `retrodoc roles`
   first): the LLM reads only `model`-role files, in batches of ~12k chars, and returns entities (name,
   description, attributes, associations); test descriptions (`describe`/`context`/`it`/`test` strings,
   `def test_*`) are extracted mechanically from `test`-role files. Saved as `.retrodoc/cache/glossary.yaml`,
   which doubles as the cache (per-file content hash).
   Smoke test on the full Rails test repo (443 model files, `qwen3.6:35b-a3b`, 24 min): 301 entities,
   190 files without entity (mostly technical classes, plausible), 5,352 test phrases from 521 test files, 4
   entities dropped for an unknown file. Business names come out well (Contract, Company, Worksite,
   FormContract, associations included), but: the LLM also lists *referenced* classes as entities of the
   file that mentions them (`Company`, `User` under `actions/company_user_actions.rb`), so the same entity
   can appear under several files — fixed by `Glossary::merged_entities` (merge by name, home entry = the file named
   after the entity, e.g. `company.rb`; 301 → 234 entities on the Rails test repo, `Contract` merged from 13 files); and `app/models/actions/*` are action/form
   objects the role rules call `model`, so some "entities" are really behaviours. Not done: verbs (step 3), existing docs/commit
   messages as glossary sources.
3. Entry points and outputs inventory. **Implemented** (`entry_points.rs`, `retrodoc entry-points`, needs
   `retrodoc roles` first): the LLM reads only `entrypoint`-role files, several small files per call, and
   lists each entry point (kind, name, verb, resource, description) with its observable outputs. Saved as
   `.retrodoc/cache/entry-points.yaml`, which doubles as the cache (per-file content hash). The same entry
   point can appear twice (route in a routes file, action in its controller); linking them is left to step 4.
   Smoke test on the Rails test repo (143 `entrypoint` files, `qwen3.6:35b-a3b`, ~40 min over three attempts):
   469 entry points (465 HTTP routes, 2 webhooks), 18 files without any; outputs are rich (434 responses, 188
   db writes, 20 emails, 18 events, 17 files, 13 external calls). Lessons: (1) a routes file asked for one entry
   per route overflows the 8,192-token answer and the 120 s client timeout (retries restart the generation) —
   the prompt now asks one entry per resource/namespace for routing files, and `llm.timeout_secs` makes the
   timeout configurable; (2) batches are now saved one by one, so a failed run resumes. **Known gap, fixed afterwards:** files
   were cut at 5,000 chars, so a large controller (`contracts_controller.rb` is 34 KB) was seen at ~15% and most
   of its actions were missed. Long files are now split into chunks of ~5,000 chars (on line boundaries, preferably
   before a blank line), sent as "(part i/n)", at most 8 chunks per file (the rest is logged as unread); a file is
   saved only once all its chunks are answered and its entry points are deduplicated by name. Unit-tested, not
   re-run on the Rails test repo. The glossary chunks its model files the same way (`chunks.rs`; an entity seen in several chunks is merged). The other passes (actors 3,000, use cases/confidence 4,000 per file) still truncate. No jobs/mailers either in this run
   (the role rules of that run classified them `infra`).
4. Rewire domains, then use cases, on the surface; then actors, two output levels, vocabulary criterion.
   - **4a — domains: implemented** (`surface.rs`, `retrodoc surface`, `generate` runs the surface passes first).
     The clustering prompt gets the best connected entities and the entry points by resource, with a rule to
     name domains after business concepts; layer-named domains are flagged. Unit-tested with a fake provider;
     not run end to end on the Rails test repo (the repo map of 2,300 files is out of reach locally until phase 8), only the
     surface itself was checked on real data (234 entities, 223 resources).
   - **4b — use cases from entry points: implemented** (`slices.rs`, `UseCase.entry_points`). Checked on one
     hand-built feature of the Rails test repo (`signatories_controller.rb`, real local LLM): the use case is tied to
     `PATCH /signatories/:id` and its steps cite the service it runs (`signatories/modify_user.rb`, outside the
     feature) — but all actors are "System" and the wording is still technical (4c, 4d). The slice heuristic on
     real data: the model named after the controller comes first, with some noise from same-named files
     (`lib/s_pdf/...`) and hop 2; no call tracing. Not run end to end (needs the full repo map).
   - **4c — business actors: implemented** (`actors.rs`, `retrodoc actors`, `UseCase.primary_actor`). The Rails test repo, real
     local LLM, ~1 min: 7 actors from `ability.rb` and the policies (Contract Manager, Folder Viewer, Signatory,
     External Document Provider…). On the hand-built signatories feature, the use case now reads "Contract Manager
     assigns or changes the person responsible for signing a contract", primary actor Contract Manager, step 1
     "submits PATCH request". Still technical: the remaining steps are system steps ("System — checks
     authorization"), and a purely technical use case (phone format validation) is still produced — the
     business-level narrative (4d) and the vocabulary criterion (4e) are meant to handle that. Actors are global
     (not proposed per domain as first sketched).
   - **4d — two output levels: implemented.** Each use case has a `narrative` (2-4 sentences: who does what and
     why, which business objects change, the observable result; no class/method/file/HTTP detail), asked in the
     same LLM call as the steps, with the application's main entity names given as vocabulary
     (`UseCaseContext.vocabulary`). The use case page shows it under "What happens" and folds the technical steps
     and the diagram in a `<details>` block; artifacts without a narrative render as before. On the signatories
     feature: "The Contract Manager initiates a change for the person assigned to sign a specific contract…
     ongoing signature transactions … are immediately canceled. Finally, relevant parties are notified…".
     The narrative is not checked against the code (the confidence pass still scores the steps only) — that is
     what 4e is for.
   - **4e — business-language criterion: implemented** (`vocabulary.rs`, `UseCase.business_language`). A
     deterministic score (no LLM call, recomputed every run) next to the confidence: 0 without a narrative; −0.15
     per code-level token in it (identifiers, paths, calls, HTTP verbs, unknown multi-word class names, words like
     "controller"/"params"), capped at −0.6; −0.3 if it names no known entity or actor; −0.2 when actors are known
     but the use case has neither entry point nor primary actor (an internal helper). Shown on the use case page
     and in the debt report ("Technical-sounding use cases", below 50%). On the Rails test repo: the real signatories
     narrative scores 100%, a hand-written code-level one 40% with the offending words listed. It is a hint, not a
     verdict: it cannot tell a bland narrative from a good one, only that it speaks code or names nothing of the
     business. An LLM judge could complement it later if this proves too coarse.

Validation: re-run on the Rails test repo and autoroute; compare actors, domain names and use-case titles by hand
(a reader who doesn't know the code should be able to say what the product does).

### 7.2 Phase 8 — scalability & cost control (open)

Observed: the cost is linear in LLM calls — one per file, per directory, per domain unit, per feature, per
use case, plus one **per use case** for confidence (about a third of all calls). 26 files already took 31
min locally; the ~2,300-file Rails test repo is out of reach in this form.

Delivered: progress reporting for long passes (`progress.rs`: one log line per unit with rank, %, elapsed
and ETA) and `HeartbeatProvider` (logs "still waiting for the LLM" every 30 s), to tell a slow call from a
stuck run.

Directions:
1. ~~Bound the depth~~ — **delivered as an opt-in budget**: `generate --max-files N` (or `ingest.max_files`) keeps
   the N best-ranked source files (`ranking.rs`: role weight × (1 + ln(1+commits) + ln(1+files mentioning its
   name)), no LLM call, 80 ms for 325 files) and drops the rest before the repo map; they are listed in
   `.retrodoc/cache/scope.yaml` and in the report ("Not analysed (file budget)"). Phase 7's surface extraction
   already bounds the glossary and entry-point passes by role. Checked on the Rails test repo services/policies/controllers:
   contracts/folders/worksites controllers on top, front-end configs last; base classes (`application_controller`)
   rank high because everything mentions them — not damped yet. Not yet run through a full `generate`.
2. Batch calls — **delivered for file summaries**: small files (≤ ¼ of `llm.batch_chars`, default 6000; 0 disables)
   are summarized together, up to 8 files / `batch_chars` per request, answered as JSON; a file the answer
   misses falls back to its own request. Confidence is batched too: up to 5 use cases of the same feature share one request (code shown once); a use case
   the answer misses falls back to its own request. Quality of batched summaries vs one-by-one is not yet compared on a real
   model.
3. ~~Confidence pass optional or sampled~~ — **delivered**: `generate --no-confidence` skips it, `--confidence-sample N`
   scores at most N unscored use cases (evenly spread; a later run scores N more). Unscored items show up in the report.
4. ~~Configurable concurrency~~ — **delivered for the repo map**: `llm.concurrency` (default 1) bounds the calls in
   flight for file summaries and, level by level (same depth), directory summaries. The other passes stay
   sequential (features/use cases share slug state and fingerprints; glossary/entry points batch serially) —
   next candidates once measured on OpenRouter.
5. ~~Estimated call count before running, resumable interrupted run~~ — **delivered**: `generate` prints the
   repo map pass's expected calls and characters up front (`estimate_repo_map`, from the caches; exact on a first
   run, a lower bound for directories afterwards; the later passes depend on its output, so they are not
   estimated). A failed LLM call in the features or use cases pass now saves the units done so far (unreached
   units keep their previous results), so the rerun resumes there.
6. ~~Directory summaries regenerated on every run~~ — **delivered**: cached in `repo-map.json` by the hash of
   the listing sent to the LLM (children's summaries), so a changed file only invalidates its folder and
   ancestors (unit-tested, not yet measured on a large repo).

First full `generate` on a real repo (2026-10-02, the Rails test repo, sparse-checkout `app/services` +
`app/policies` + `app/controllers`: 325 source files; `qwen3.6:35b-a3b`, `concurrency = 2`, default batching):
completed in about 2h20 (the progress log was lost, so per-pass timings are not recorded). The estimate announced
201 file calls + 89 directory calls for 325 files (batching: ~38% fewer file calls); the repo map pass was at 67% after
11 min. Result: 7 domains, 77 features, 183 use cases, overall confidence 80%, business language 75%, 269 doc files.
Quality: domains are business-flavoured (`contract-lifecycle`, `e-signature-integration`, `organizational-management`)
except `api-integration-layer`; 13 features ended with no use case (answers unparseable twice, or nothing derivable);
the SSO/Keycloak/user-sync use cases read as internal helpers (flagged by the business-language score); some narratives
leak column/class names (`from_company`, `companiesfolder`). Unverified: whether `concurrency = 2` gained anything,
since Ollama has no `OLLAMA_NUM_PARALLEL` set.

Validation: measure calls and wall time before/after on the Rails test repo `app/presenters/` (baseline above), then
on a larger sparse-checkout (e.g. `app/models` + `app/services`).

### 7.3 Phase 9 — ask the documentation (open)

Idea (2026-10-02): once the documentation is built, the ideal consumer is an agent that can be questioned
about how the application works ("what happens when a contract is signed?", "who can cancel a
subscription?"), answering from the generated docs, the collected docs and, when needed, the code.

This is where retrieval belongs. The generation pipeline is exhaustive and deterministic (100% of files
covered, fingerprint-based incremental re-run), so a general RAG layer there would add state and
non-determinism for no gain. Question answering is the opposite: open-ended questions over a corpus too big
for one context, so retrieval is the core of the feature.

Design: hierarchical navigation by an agent with tools, rather than a flat chunk index.
1. Route by structure: domains → features (from `domains.yaml`/`features.yaml` and the repo map summaries)
   to find the relevant zone.
2. Read the use cases and steps of that zone; they already cite files.
3. Go down to the code (and git history) only when the question is technical or the docs are uncertain.

Tools given to the agent (names indicative): `list_domains`, `get_feature`, `get_use_case`, `search_docs`,
`read_source`, `git_log`. The LLM navigates; no embeddings at the start.

Index: BM25 (e.g. `tantivy`) over the generated artifacts and the collected Markdown docs (small corpus,
rebuilt in seconds), plus the surface-extraction glossary from phase 7 for query-vocabulary matching. Code
search is lexical (identifiers match well) over function-level chunks. Embeddings only if the lexical
search fails on business vocabulary that differs from the code's; measured, not assumed.

Requirements:
- **Cite sources**: every answer lists the features, use cases and files it relied on, so it is checkable.
- **Use confidence**: if the answer rests on a section below the threshold (50%), say so and verify against
  the code instead of repeating the doc.
- **Freshness**: compare the fingerprints of the cited files with the current tree; warn when the docs are
  older than the code, and fall back to reading the code.
- **Say "not documented"** rather than guess when nothing relevant is found; such misses can feed the
  documentation debt report.

Technical impact:
- `LlmProvider` has no tool calling today (`complete` takes plain text messages and returns text). It must
  be extended (OpenAI-compatible `tools`/`tool_calls`, which OpenRouter supports, though not every model
  does), or the agent loop can use a JSON-action protocol on top of `complete` as a fallback for local
  models. Decide when starting.
- New crate `retrodoc-agent` (tool loop, retrieval, prompts), depending on `retrodoc-llm`,
  `retrodoc-core` and `retrodoc-ingest`; new CLI commands `ask "<question>"` and an interactive `chat`.
  Read-only: no write to the target repo. Tested with a fake provider and scripted tool calls, like the
  pipeline passes.

Risks: the agent repeats the errors of the generated docs with confidence, hence the dependency on phases 7
and 8 and on the confidence score; the context budget of a local model limits the number of tool round
trips; answers are hard to evaluate automatically, so start with a small hand-written question set per
smoke-test repo (the Rails test repo, autoroute) and compare answers with and without code access.

Steps, each shippable:
1. Read-only retrieval tools + BM25 index over the artifacts and collected docs, exposed as a debug
   command (`retrodoc search`) to judge retrieval quality without any LLM.
2. Tool calling support in `retrodoc-llm` (or the JSON-action fallback).
3. `retrodoc ask` with citations, confidence and freshness warnings.
4. Interactive `chat` with conversation memory; code and git history tools.

### 7.4 Smaller open items

- `.erb` view templates are not classified as `Source` by the walker (1,253 files on the Rails test repo).
- Retry on an empty/invalid LLM answer fixes symptoms; the underlying causes (context length, truncated
  JSON) depend on the server config — see the local LLM setup notes.
- Truncation: the entry points and glossary passes now chunk long files; `MAX_CHARS_PER_FILE` (4,000) in the use cases/confidence
  passes and the repo map still cut long files, so late code can never be cited there.
