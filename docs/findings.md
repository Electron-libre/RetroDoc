# Findings log

The dated findings of the smoke tests, measures and reviews that shaped RetroDoc from 2026-10-01 to 2026-10-14,
moved from the former `PLAN.md` (§7). It is a log: the current behaviour is in `docs/crates/` and
`docs/ARCHITECTURE.md`, the planned work in `ROADMAP.md`, the decisions in `docs/adr/`, the later findings in the
closed issues of `issues/done/`.

The phases it names are the milestones of `ROADMAP.md`:

| Phase | Milestone |
|---|---|
| 1 | Foundation |
| 2 | Repo map |
| 3 | Domains |
| 4 | Features, use cases and diagrams |
| 5 | Confidence and debt report |
| 6 | Writing and CLI ergonomics |
| 7 | Business-level documentation |
| 8 | Scalability and cost control |
| 9 | MCP server over the generated docs, then Ask the documentation |
| 10 | Top-down redesign |
| 11 | A large application, one bounded context at a time |

## 1. First smoke tests (2026-10-01)

Smoke tests were run with `qwen3.6:35b-a3b` on a local Ollama (AMD iGPU, 32k context,
`reasoning_effort = "none"`), on throwaway `git worktree`s in `~/retrodoc-runs` (see docs/dev/commands.md for the CLI commands):

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
  answer with zero use cases is now retried once, and a warning is logged only when the second answer is empty too (a first empty answer the retry recovers is a `debug` line).
- **Use case answers rejected for a text field given as an object.** On a small Rails repo, about 7 of 35
  features hit `invalid type: map, expected a string`: the model returned `primary_actor` as a
  `{name, kind}` object, copying the shape of a step's `actor`, and the strict parse dropped the whole feature.
  `description`, `primary_actor`, `narrative` and a step's `action` now accept an object, turned into text
  (its `name`, else its string values but `kind`/`type`; `debug!` logs it). Rerun of a feature that failed
  on every attempt: 1 call, no warning, no retry; the full first-run WARN count was not re-measured, and the
  model is not deterministic, so this is one sample. Truncated JSON (`finish_reason=length`) is a separate
  problem, not covered.

## 2. Phase 7 — business-level documentation

Observed: the documentation is mostly technical and the business domain is barely perceptible. Example
(the Rails test repo): a use case "Format children companies for UI selection" has actor "Developer", five steps that
restate method calls (`children_companies`, `map`), and a sequence diagram between a developer and a Ruby
object. The top domain is named "presentation-layer" — a technical split, the very risk listed in the risks of `PRODUCT.md`.

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
   **Implemented** (`roles/`, `retrodoc roles [--force]`, rules in `.retrodoc/cache/roles.yaml`).
   Smoke test on the full Rails test repo (4,181 files, `qwen3.6:35b-a3b`, ~1m30 per call): ~92% of files
   get a role (~350 unclassified: dotfiles, `Rakefile`, coffee scripts), but two runs disagree on
   borderline folders (mailers/jobs entrypoint vs infra, presenters logic vs view) and one run out of five
   returned unparseable JSON twice — hence the saved, editable rules. autoroute not tried yet. Not done: wiring the roles into `generate`. The second pass over the unmatched
   files was added on 2026-10-30 (LNG-16 to LNG-19): on the saved runs, 0 of 443 (linkding), 0 and 5 of 791
   (lobsters) and 163 of 4,181 (the Rails test repo, ~80 of them the application's code: API blueprints and
   decorators, components, coffee scripts) are unmatched.
2. Models and glossary inventory. **Implemented** (`glossary/`, `retrodoc glossary`, needs `retrodoc roles`
   first): the LLM reads the business files (formerly only `model`-role files), in batches of ~12k chars, and returns entities (name,
   description, attributes, associations); test descriptions (`describe`/`context`/`it`/`test` strings,
   `def test_*`) are extracted mechanically from `test`-role files. Saved as `.retrodoc/cache/glossary.yaml`,
   which doubles as the cache (per-file content hash).
   The files read are the ones the business-files pass located (`business_files/`, `retrodoc business-files`,
   ADR 0025), not the `model` role (used only when there is no list), and `generate` logs when the surface still
   has no entity. The ADR 0018 fallback on `logic`/`entrypoint` files is gone.
   Smoke test on the full Rails test repo (443 model files, `qwen3.6:35b-a3b`, 24 min): 301 entities,
   190 files without entity (mostly technical classes, plausible), 5,352 test phrases from 521 test files, 4
   entities dropped for an unknown file. Business names come out well (Contract, Company, Worksite,
   FormContract, associations included), but: the LLM also lists *referenced* classes as entities of the
   file that mentions them (`Company`, `User` under `actions/company_user_actions.rb`), so the same entity
   can appear under several files — fixed by `Glossary::merged_entities` (merge by name, home entry = the file named
   after the entity, e.g. `company.rb`; 301 → 234 entities on the Rails test repo, `Contract` merged from 13 files); and `app/models/actions/*` are action/form
   objects the role rules call `model`, so some "entities" are really behaviours. Not done: verbs (step 3), existing docs/commit
   messages as glossary sources.
3. Entry points and outputs inventory. **Implemented** (`entry_points/`, `retrodoc entry-points`, needs
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
   saved only once all its chunks are answered and its entry points are deduplicated by name. Where to cut is not hard-coded per language: the roles call also returns `chunk_boundaries` (per language: extensions + a regex matching the line that *starts* a module/class/function, saved in `roles.yaml`, editable); the chunker cuts before the last such line in the second half of a chunk, else after a blank line, else at any line (old `roles.yaml` without the field: `retrodoc roles --force`). The proposed regexes are checked mechanically (`chunk_check.rs`): a language-agnostic probe finds the definition-like lines (and the attribute/decorator line above) in the 20 largest files of the extension; below 90% coverage the LLM gets up to 3 focused calls listing the lines it missed, the best regex is kept, and dropped below 50% (blank-line fallback). On autoroute with `qwen3.6:35b-a3b`: before the prompts spelled out the regex dialect (no lookaround, one line, JSON-escaped backslashes; a non-compiling regex is also reported back with the compiler's message) the final coverage was 57-78% and the model kept proposing lookaheads; after, 72%, 78% and 100% on three runs. The check keeps the output measured instead of trusted. It also measures *breadth* (a regex matching over 40% of the sampled lines is rejected and the problem is sent back to the LLM): on the Rails test repo the first version accepted a js/ts regex ending in `|` (an empty alternative matching every line) at "100%". The Rails test repo `contrat` (`roles --force`, ~1m45): rb 98%, js/ts 100% after 3 fix calls (two earlier fixes used lookaheads and were reported as 0%). `entry-points` on `contracts_controller.rb` alone (34 KB, 65 `def`, 7 chunks, ~2 min): 32 entry points (28 of its 35 public actions named) against 9 before chunking. Not re-run on the whole repo. Run on the whole the Rails test repo (194 `entrypoint` files, `qwen3.6:35b-a3b`, ~45 min + 14 min for the files redone): 745 entry points in 190 files at the first full pass (810 in 194 after the fixes below) against 469 in 143 before chunking (on the 132 files in both: 624 vs 450; `contracts_controller.rb` 9 → 37, `folders_controller.rb` 6 → 21, `config/routes.rb` 11 → 174). Lessons: (1) the `(part i/n)` marker was copied by the model into the cited file path (34 entry points dropped as "unknown file"): `strip_part_marker` removes it; (2) a batch whose answer overflows the 8,192-token limit twice (a batch of 9 routing files, 28k chars) used to be skipped for good, `config/routes/admin.rb` staying unread: the batch is now retried file by file (entry points and glossary), after which all 194 files are read: 810 entry points (133 files in common with the old run: 685 vs 464; `admin.rb` 14 → 30, `routes/contracts.rb` 19 → 31); (3) `config/routes/api.rb` still yields 0 entry points (6 before), cause not investigated, and 13 files in all have none (jobs, concerns, a few controllers — plausible but unchecked). `worksites_controller.rb` (10 chunks) is read on 8. The glossary chunks its model files the same way (`chunks/`; an entity seen in several chunks is merged). The use cases and confidence passes no longer send the head of a long file: `Splitter::excerpt` shows the first chunk plus the ~1,200-char chunks that matter, with absolute line numbers and the omitted ranges marked (`… (lines a-b omitted)`). Use cases: chunks naming the identifiers of the feature's entry points (`send_contract` in `POST /contracts/:id/send_contract`), then the words of the feature's text; confidence: chunks holding the lines the steps cite. Without any focus it is the old head truncation. Same per-file (4,000) and per-prompt (30,000) budgets. Unit-tested only, not yet run on the Rails test repo. The actors pass truncated too (12 authorization files, 3,000 characters each) until 2026-10-30: measured on the saved runs, no selected file was over 3,000 characters, but on the Rails test repo most of the candidate files were never sent; it now reads them within one 40,000-character budget, a file over 8,000 as its excerpt, and warns of the files left unread (`specs/actors-reading.md`). No jobs/mailers either in this run
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
     The narrative was not checked against the code (the confidence pass scored the steps only); since
     2026-10-30 the confidence call judges it with the doubtful steps (ADR 0038,
     `issues/done/narrative_checked_against_cited_code.md`, measure there). Measured on 40 use cases of two
     applications, it catches almost no wrong narrative: 8 wrong claims in 200 by hand (a wrong condition, scope,
     reason or caller), but the model sees only the cited code, so it cannot tell a claim true in the code around
     it from an invented one, and it judged no contradicted claim `unsupported`. A wrong narrative is not found
     from the cited code alone; reading the code around it, or checking the steps' citations, is what is left.
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

## 3. Phase 8 — scalability & cost control

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
   Since ADR 0026 the checks are mechanical first and `--confidence-sample N` asks the LLM for a second opinion on N use cases that pass them.
4. ~~Configurable concurrency~~ — **delivered for the repo map**: `llm.concurrency` (default 1) bounds the calls in
   flight for file summaries and, level by level (same depth), directory summaries. The other passes stay
   sequential (features/use cases share slug state and fingerprints; glossary/entry points batch serially) —
   next candidates once measured on OpenRouter.
5. ~~Estimated call count before running, resumable interrupted run~~ — **delivered**: `generate` prints the
   repo map pass's expected calls and characters up front (`estimate_repo_map`, from the caches; exact on a first
   run, a lower bound for directories afterwards; the later passes depend on its output, so they are not
   estimated). 2026-11-05: removed with the repo map pass (`d5255d5`); nothing estimates a run any more, the
   milestone "Pre-run cost estimate" of `ROADMAP.md` brings it back for the whole pipeline. 2026-11-05: delivered
   (`specs/run-estimate.md`): `generate` prints the range of calls and tokens of every pass before its first call,
   `--estimate` alone; the end-of-run recap sets each pass against it and `usage.json` keeps it (TOK-6 to TOK-8). A
   failed LLM call in the features or use cases pass now saves the units done so far (unreached
   units keep their previous results), so the rerun resumes there.
6. ~~Directory summaries regenerated on every run~~ — **delivered**: cached in `repo-map.json` by the hash of
   the listing sent to the LLM (children's summaries), so a changed file only invalidates its folder and
   ancestors (unit-tested, not yet measured on a large repo).

7. **Token accounting** — step 1 delivered, step 2 open. Step 1: `CompletionResponse` carries an optional `usage`
   (`prompt_tokens`, `completion_tokens`, which OpenRouter, Gemini and Ollama return; a missing or malformed block
   is `None`, never estimated). `UsageProvider` (a wrapper like `HeartbeatProvider`) sums calls and tokens per pass
   and per model in a `UsageTracker`, and every command that calls the LLM (`generate`, `roles`, `glossary`,
   `entry-points`, `actors`) ends with a recap: calls, tokens in/out and time per pass, per model when there are
   several, and a warning when some calls reported no tokens. The last 20 runs are kept in
   `.retrodoc/cache/usage.json` (outside the generated docs, so reruns stay no-ops). Failed attempts and retries
   inside the provider report no tokens, so the sums are a lower bound against a flaky server. Decided against
   prices: the recap gives tokens only, the amount is up to the reader (ADR 0015). Unit-tested, and smoke-tested on
   a small Rust repository (17 files, `qwen3.6:35b-a3b` locally): the first `generate` made 37 calls for 22,881 tokens
   in and 7,830 out in 4m34s (use cases 8 calls and 1m57s, repo map 13 calls); the rerun was a no-op for the docs but
   still made 2 calls (a feature for which the LLM answers "no use case" was retried on every run, which the recap made
   visible; since then a clean empty answer is remembered, see ADR 0005). Not yet measured on a large repo. Step 2, once real measurements exist (the saved history
   provides them): a pre-run estimate for the whole pipeline. `estimate_repo_map` already gives calls and characters for the repo map (~4
   characters per token); the later passes depend on earlier outputs, so extrapolate with measured ratios (the Rails test repo:
   325 files gave 77 features and 183 use cases, ~0.24 and ~0.56 per file). Output tokens (use case JSON, up to 8,192
   per call) are the hardest to predict; an estimate made before any measurement can be off by 2-3x.
   2026-11-05, measured (`issues/done/pass_ratios_from_usage_history.md`, no LLM call): the first complete `generate` of
   21 saved runs (delivery_router, healthchecks, linkding, Lobsters, deepseek-flash, 2026-10-27 to 11-03) and a
   large private Rails app as indicative only (local model and deepseek-flash, 2026-10-15 to 10-23, out of the medians) give `crates/retrodoc-pipeline/data/pass-ratios.yaml`, rebuilt by the example
   `pass_ratios`. Medians across the four applications: the six fixed passes (`sources`, `brief`, `roles`,
   `business-files`, `actors`, `domains`) about one call each and ~26k prompt / ~3.8k completion tokens a run;
   `business-read` 0.16 calls (0.10-0.26) and ~440 prompt tokens per 1,000 characters read (up to the 40,000 a
   file is read), `entry-points` 0.13;
   `features` 1.0 call and ~2.6k tokens per domain; `use-cases` 1.02 calls, ~10k prompt (2.9k-11.8k) and ~750
   completion tokens per feature, the largest share; `rules` 3 calls and ~6.7k per feature (measured on two
   applications only, the benchmark runs turn it off); `confidence` 0.30 calls and ~770 tokens per use case;
   `reconcile` 0.13 calls per reconciled item; no call without usage. Yields: 2.4 entry points per file read, 7 domains
   (3.5-9.6), 5.5 features per domain (2.4-7.9), 2.7 use cases per feature (1.5-3.4). The per-file ratios above
   predate the top-down redesign: the extrapolation now goes by domain and feature, not by file.
   2026-11-05, same runs: chaining the yields multiplies their extremes (13 to 256 use cases for one repository), so
   the table also gives each pass per source file (`source-hashes.json`, the input known before a run): `use-cases`
   0.22 calls (0.09-0.94) and ~2.3k prompt tokens (0.8k-2.7k) per source file, `confidence` 0.18 (0.08-0.39),
   `business-read` 0.09 (0.04-0.21); the small Ruby library (9 source files) gives every upper bound, its fixed
   passes weighing on few files.

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

## 4. Phase 9 — ask the documentation

Idea (2026-10-02): once the documentation is built, the ideal consumer is an agent that can be questioned
about how the application works ("what happens when a contract is signed?", "who can cancel a
subscription?"), answering from the generated docs, the collected docs and, when needed, the code.

This is where retrieval belongs. The generation pipeline is exhaustive and deterministic (100% of files
covered, fingerprint-based incremental re-run), so a general RAG layer there would add state and
non-determinism for no gain. Question answering is the opposite: open-ended questions over a corpus too big
for one context, so retrieval is the core of the feature.

Update (2026-10-06): the first consumer is an external agent through a **local MCP server** (stdio,
`retrodoc mcp`, official `rmcp` SDK), see `issues/done/mcp_server.md`. The calling agent is the LLM, so the server
is deterministic and LLM-free (ADR 0016): steps 1 and 3 below become MCP tools, step 2 (tool calling in `retrodoc-llm`)
is not needed for it, and the internal `ask`/`chat` remain optional later. A static agent index
(`llms.txt`-style) is also generated with the docs.

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
- **Freshness**: compare the hashes the last `generate` recorded for the cited files with the current tree; warn when the docs are
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
   command (`retrodoc search`) to judge retrieval quality without any LLM. **Implemented** (`retrodoc-mcp`,
   hand-written BM25; the glossary is indexed too); checked on the `delivery_router` smoke repo: right use case found, but no synonym matching and no relevance threshold (an undocumented topic still returns entries sharing a common word).
   The MCP server (`retrodoc mcp`, seven tools, answers in Markdown with confidence and cited files) is also
   **implemented**; its stop-word and half-of-the-words filter does not fix that case with two-word queries.
   Fixed (2026-10-31, `issues/done/search_relevance_threshold.md`, `specs/docs-search.md`): a hit must hold more than half
   of the query's IDF weight. On 42 hand-written questions over three saved runs, no floor on the score separated
   documented from undocumented topics; this one keeps 19 of 21 documented (the two lost use a word the docs don't
   have: no synonyms) and answers "not documented" to 17 of 21 undocumented.
   Freshness warnings are implemented too (current hash of each cited file against `source-hashes.json`, which only a `generate` that went through every pass writes, so a run that failed after the repo map leaves it describing the docs still on disk; until the first complete run after upgrading, files are not judged). `read_source` and `git_log` are implemented as well, limited to the files the docs cite (no `..`, no symlink at all, no binary or very large file, long lines cut, no author names, merge commits left out).
2. Tool calling support in `retrodoc-llm` (or the JSON-action fallback).
3. `retrodoc ask` with citations, confidence and freshness warnings.
4. Interactive `chat` with conversation memory; code and git history tools.

## 5. Smaller open items

Followed since 2026-10-30 by `issues/done/roles_second_pass_and_promoted_sources.md` (the source extensions below and the
unclassified files of §2 step 1), `issues/done/actors_read_long_authorization_files.md` (truncation),
`issues/done/narrative_checked_against_cited_code.md` (§2 step 4d) and
`issues/done/search_relevance_threshold.md` (§4 step 1). Dropped: the labels of the view templates (below,
`issues/done/view_template_labels.md`), the server-dependent causes of empty answers (not
fixable in the code), the damping of base classes in the file ranking (§3 direction 1; the budget is opt-in and the
contexts path bounds a large run), the verbs and the docs and commits as glossary sources (§2 step 2; the evidence
retrieved per unit of ADR 0019 brings the docs and commits, the entry points the verbs), and the routing file of the
large application that gave no entry point (§2 step 3; not reproducible without it).

- Source extensions: the walker's built-in list (`FileKind::from_path`) only knows common languages. The roles
  pass now also asks the LLM for `source_extensions` (saved in `roles.yaml`); `generate` promotes the matching
  `Other` files to `Source` right after it (`RoleRules::promote_sources`; the `actors` command does it from the
  saved `roles.yaml`). Checked through `generate` on an Elixir fixture with a fake LLM (`tests/unknown_stack.rs`),
  not yet on a real repo; `scan` and `roles` apply the saved `source_extensions` too and count the promoted files.
- `.erb` view templates are meant to stay `FileKind::Other` (1,253 files on the Rails test repo): the file budget and
  the actors pass select on `FileKind::Source` only, never on role. On that repository the roles pass listed `erb`,
  `jbuilder`, `scss` and `css` in `source_extensions` (1,386 files, ~38 % of the source set); since 2026-10-30
  (`issues/done/template_extensions_never_promoted.md`) the promotion leaves out the extensions of templates, markup,
  stylesheets and data whatever the answer (LNG-20), `vue`, `svelte` and `astro` kept (code and markup together).
  They keep their `view` role (used by `classify`).
  Dropped (2026-10-30, `issues/done/view_template_labels.md`): extracting their labels. Measured without LLM on the
  pinned benchmark repositories: the visible text of the templates (text nodes, `label`/`placeholder`/`title`/`alt`/
  `aria-label`/`value` attributes, quoted strings of the template tags such as `f.label :x, "Text"`) against the
  words of `benchmark/<repo>/reference.yaml`, the files read being the business files and entry-point files of the
  `source_run` of the frozen inputs. Of the reference words those files lack, the templates hold 17 of 67 (linkding,
  46 templates) and 21 of 38 (Lobsters, 130 templates), mostly generic words (`later`, `whom`, `tell`); 1 and 7 of
  them are in no other source file nor doc, and the business words among them are few (`bookmarklet`, `subscribe`,
  `aggregation`). Too little to expect a change of the domains or features without a paid benchmark; to reopen on
  a repository whose labels hold its business words.
- Retry on an empty/invalid LLM answer fixes symptoms; the underlying causes (context length, truncated
  JSON) depend on the server config — see the local LLM setup notes.
- Truncation: the entry points and glossary passes chunk long files and the use cases/confidence passes show the relevant chunks of a long file; the actors pass still cuts long files.

## 6. Phase 10 — top-down redesign

Review of 2026-10-07: the pipeline climbs from the code to the business and its cost grows with the file
count. Findings from the code: the existing docs reach one prompt as their first line only, commit
messages are not collected, the test descriptions are extracted but read by no LLM pass, schema, i18n and
`.feature` files are not sources, the features pass sees only file summaries, the model has no place for
business rules, and the debt report never compares with the existing docs. The cost comes from the "every
file in a domain" invariant (ADR 0004): every file and directory is summarized, and a file is read by the
LLM in up to five passes.

Decisions (accepted ADRs):
- [0019](docs/adr/0019-product-brief-and-non-code-evidence-first.md): collect the non-code signals
  mechanically and write an editable product brief (`product.yaml`) that frames every later prompt; each
  unit retrieves its relevant doc sections, commits and test phrases with BM25. The retrieval per unit is on by default (`brief.evidence`) since the replays
  of `issues/done/brief_injection_experiments.md` (2026-10-28) showed merged duplicate features and more
  business-like use cases for about a tenth more prompt tokens.
- [0020](docs/adr/0020-cover-behaviours-not-files.md) (supersedes 0004): every entry point belongs to one
  use case; domains and features are built from entry points and entities; files reached by no behaviour
  are reported as technical support; the LLM repo map becomes optional; confidence is mostly
  deterministic.
- [0021](docs/adr/0021-business-rules-as-a-model-object.md): `BusinessRule` in the model, with its
  evidence in code, tests and docs.
- [0022](docs/adr/0022-tree-sitter-for-code-structure.md) (supersedes 0013 for the languages it covers):
  definitions, signatures and references from tree-sitter.
- [0023](docs/adr/0023-structured-outputs-and-a-model-per-pass.md): JSON schema in the request, and an
  optional model per pass.
- [0026](docs/adr/0026-check-use-case-steps-mechanically-before-asking-the-llm.md): the confidence pass checks
  the steps mechanically and asks the LLM only about the doubtful ones.
- [0027](docs/adr/0027-parse-to-cut-long-files-and-opt-in-short-bodies.md): long files of the five parsed
  languages are cut at their definitions; the opt-in short method bodies were removed after a measure (2026-10-27).
- [0028](docs/adr/0028-read-the-business-files-once-for-entities-and-entry-points.md): `generate` reads the business
  files once for the glossary and the entry points (`[pipeline] business_read`, on by default).

Steps, in order, one issue each:
1. `issues/done/quality_benchmark.md`: a reproducible measure of the docs' quality, before any other step.
2. `issues/done/product_brief.md` (then `issues/done/locate_business_files.md`, which takes the brief as input).
3. `issues/done/structured_llm_outputs.md` and `issues/done/model_per_pass.md`.
4. `issues/done/behaviour_coverage_domains.md`: the pivot, shipped in steps, the old path kept behind a setting
   (`[pipeline] domains_from`) until the benchmark favours the new one. In progress: the old path is measured,
   the domains and the features can be built from the surface (`generate` runs end to end in that mode); the
   entry points no feature holds are placed by a small call or listed in the coverage report, as are the files
   reached by no behaviour. Benchmark of 2026-10-10: on a 178-file application `surface` makes 55% fewer calls
   and takes about 45% less time at par quality; on a 9-file library its features are worse. The default became
   `surface` on 2026-10-28 (`issues/done/surface_domains_default_decision.md`, ADR 0034); the cost of the use cases pass, the deterministic
   confidence and the single read per file are queued as their own issues.
5. `issues/done/tree_sitter_structure.md`: long files cut at parsed definitions for five languages, short method bodies as an option (ADR 0027); the parsed references moved to `issues/done/parsed_references_for_ranking_and_slices.md`.
6. `issues/done/business_rules.md`.
7. `issues/done/docs_reconciliation_debt_report.md`: every domain, feature, use case and rule matched by BM25 to
   the existing doc sections, then judged covers / contradicts / unrelated by the LLM (pass `reconcile`, ADR 0033);
   the debt report counts the coverage per domain and lists the contradictions and the undocumented items, the
   item pages link their doc sections. Not yet measured on a repository with functional docs.

Expected: a cost that grows with the business surface (entry points, entities, use cases), not with the
repository. That is an estimate, to be measured with step 1 and `usage.json`.

Follow-up of step 6: the rules are stated per feature, so some are duplicated, and storage plumbing comes out
as rules (`issues/done/rules_deduplication_and_noise.md`); the pass stays off by default.

## 7. Project review of 2026-10-12

A review of the whole project after the redesign (234 commits, about 38k lines of Rust, 28 ADRs).

Holds: the one-way crates, the YAML artifacts, the incremental re-run, and the move from files to behaviours
(ADR 0020), which is what makes the cost follow the business surface. The measuring tools (benchmark, replays
without LLM, token recap per pass) are what let the changes below be decided on figures.

Measured since the baseline of §6 step 1 (details in the closed issues):

| Change | Figure |
|---|---|
| `surface` against `files`, linkding, deepseek | 110 calls and 6.4 min against 222-283 and 9.7-14.4; judged domain recall / precision 56% / 88% against 33-39% / 41-75%; judged feature recall at par (62% against 65%), precision 34% against 23-41%; narratives in business language 87% against 70-85% |
| `surface` against `files`, a 9-file library | features worse (judged precision 45% against 96% with deepseek): the entry points pass lists constructors and internal collections |
| Strict JSON schema | unparseable answers 6 to 0 over the measured runs |
| Mechanical confidence checks | 4 confidence calls instead of 8 on `delivery_router` |
| Single read of the business files | 36% fewer calls and 33% fewer input tokens for the two reading passes on linkding |
| Business rules, linkding, deepseek | 649 rules for 66 features, 450 backed by code alone; the pass is about 42% of the input tokens of the run (1.18M in, 242k out) |

Not reached yet:
- No complete run on a large application since 2026-10-02 (done since: see §8).
- Feature precision stays low on the application benchmark, and the judge overstates it (by 12 to 33 points on
  the audited runs).
- Run-to-run spread: 13 to 21 entry points on the 9-file library, 64 against 96 between two identical runs
  of the entry points pass on linkding; the requests set no sampling temperature.
- The use cases and the rules read the same code: about 80% of the input tokens of a run with the rules on.
- Two paths (`files`, `surface`, the default since 2026-10-28) were maintained together; `files` is refused
  since ADR 0035 and its code removed by `issues/done/single_pipeline_path.md` (`body_lines` removed, `rules` on by default since
  2026-10-27, `brief.evidence` on by default since 2026-10-28).

Next work, in the order of `issues/queue.md`:
1. `issues/done/validate_on_large_application.md`: one complete run on a large application, read by the user (done in
   slices, §8).
2. `issues/done/sampling_temperature_for_steadier_runs.md` and `issues/done/benchmark_passes_on_fixed_inputs.md`: a steadier
   and cheaper measure, with references a human has checked (done: one pass replayed on frozen inputs,
   `pass_replay`; a second application is `issues/done/second_application_in_benchmark.md`).
3. `issues/done/rules_deduplication_and_noise.md`, `issues/done/one_code_reading_for_use_cases_and_rules.md` and
   `issues/done/shorter_use_case_answers.md`: quality of the rules, then the tokens of the two largest passes.
4. `issues/done/settle_experimental_settings.md`, `issues/done/surface_domains_default_decision.md`,
   `issues/done/single_pipeline_path.md`, then `issues/done/pass_runner_and_run_context.md`: fewer options, one path, then
   the refactoring of what stays.

## 8. Phase 11 — a large application, one bounded context at a time (2026-10-12 to 13)

`issues/done/validate_on_large_application.md` tried one run on a private 1,676-file Rails application and found that
the reading stopped at 60 files and showed 150 entry points of 456: the design could not be run once on a large
application. The work that followed (`issues/done/business_files_cap_on_large_application.md`, ADR 0029):

- every pass of the `surface` path saves after each answer, artifacts are written beside and renamed;
- `retrodoc contexts`: the application is mapped into bounded contexts from the names of its entry point files
  and one LLM call (`contexts.yaml`, editable), a context over 150 files is split by family, a route goes to the
  context of the controller it names;
- `[pipeline] domains_from = "contexts"`: `generate [--context a,b] [--max-files N]` documents the next contexts,
  reads only their files, keeps the inventories, features and use cases of the others (readers that add, vocabulary
  and placement per domain, actors kept), and a rerun with nothing left calls no LLM.

Result on the real application (hosted `deepseek-chat`): 8 of 35 contexts, 285 use cases, about $1.3 including
the false starts; the core of the business reads well (the user's reading), about 30% of the use cases of the first
slices are technical or back-office. The split by family is done (`issues/done/split_contexts_by_family_not_by_layer.md`: the
largest context went from 199 to 167 files); the use cases cite the entry points they cover (tags `E1`.., stored as ids), a feature of more than 12 entry points is asked in slices, and the entry points no use case cites are in `behaviour-coverage.yaml` and the debt report (`issues/done/use_cases_record_their_entry_points.md`; on a large Rails application, 92% of the use cases cite an entry point and 45% of the entry points of the features are cited by none). The `files` and `surface` paths are unchanged.

Technical and back-office use cases (`issues/done/technical_and_back_office_use_cases.md`, 2026-10-14): the features pass
marks the back-office features (`Feature.back_office`); the docs, the MCP answers, the search and the debt report set
apart those and the use cases under 0.8 of business language or 0.6 of confidence ("Administration and technical",
nothing is dropped); a use case reached only by a job, a consumer, a webhook or a public method loses its human
actor (prompt and mechanical check); in an application, the public methods no route, job or webhook reaches are left
out of the entry points; the prompt asks for one use case for a form and its submit. On the same 8 contexts, one run:
features made of public methods 21% to 5% of the use cases, human actor with no route nor command 28% to 7%, 5
back-office features found; the form screens did not drop (12% to 21%), kept in
`issues/done/merge_form_screens_into_their_use_case.md`.

Residue and spread of the map (`issues/done/contexts_residual_size_and_names.md`, 2026-10-30, hosted `deepseek-chat`):
the context over the limit and the action names taken for families depend on the roles the map starts from (none
with the roles of the local run, one context of 180 files and one false family of 3 files with hosted roles).
Three maps of the same roles: 34, 40 and 44 contexts, 30%, 10% and 10% of the files in the support, 57% to 81% of
the files in the same context from one map to another, 4 slugs in all three; the context of 180 files in the
first is cut under the limit in the next two. About 30 of the 44 names fit their files; the others are 5 technical
contexts (UI components, configuration, front-end code, base classes), 3 catch-alls and 6 one-file contexts.

Steadier map (`issues/done/context_map_spread_between_runs.md`, 2026-11-04, same roles, hosted `deepseek-chat`): the
spread came most from the merges (of the pairs of groups one map puts together, 11% to 20% are together in
another) and from the support (the migrations and initializers in the support in one map, in business contexts
in the two others). Temperature 0 alone changed little (files keeping their context 70% against 68%, merged pairs
26% against 16%). With the technical files left out before the call, a forced map keeping the slugs of the saved
contexts it mostly matches and the one-file contexts merged, three chained forced maps: 37, 30, 25 contexts, the
largest 150 files, none of one file, the technical files always in the support; files keeping their context 57%
to 78% (mean 71%), merged pairs 23% to 32%; 12 slugs in all three (4 before), 12 to 21 slugs kept from one map to
the next. Left: the support moves (21% to 34%, UI components 25% to 63%) and catch-all or layer contexts remain
(UI components, catch-alls, API, PDF, workflows).

Support and catch-alls (`issues/done/context_map_support_and_catch_alls.md`, 2026-11-04, same roles, hosted
`deepseek-chat`): without LLM, on the three maps above, half of the files moving in and out of the support were whole
groups the answer set aside in one map only, half orphan folders; the layer contexts were orphan folders (UI
components, a folder of models, a library) and the catch-alls many small groups. A layer folder mixes files that
serve different contexts: moved whole, the top-level folder of models went to one context and the workflow tasks,
written by their own context only, to the support. So the files of a layer context are placed one by one, by the
context that writes their class name, views and models nobody writes going to the support; a catch-all context is
spread the same way; the saved support is proposed to a forced call. Three chained forced maps at 639e22a: files
moving in or out of the support 109 against 538, support 25/31/31%, views in support 56 to 63% (25 to 63% before),
files keeping their context 69% to 93% (mean 81%, 71% before), 17 slugs in all three (12), no catch-all named as
such, layer contexts 3/1/0 (8/7/3). But 54/62/31 contexts (37/30/25), with 20 to 41 contexts of a single group, and
merged pairs kept together 8% to 26% (23% to 32%): the answer now leaves more groups alone. Left: groups named after
a technical word (a namespace, a verb) mapped as contexts of their own, and one catch-all named after a business
word (27 groups, 144 files) that the word list does not catch.

## 9. Benchmark baseline at the current code (2026-11-03)

Measured at dafea18 (`issues/done/benchmark_baseline_at_head.md`), hosted `deepseek-chat`, confidence on, 3 runs per
series; the earlier whole runs (2026-10-28) predate the docs language, the narrative check, the new rules judgement
and the reconciliation by identifiers. Mean (range), judged figures as the benchmark scored them at the run:

| Figure | linkding hidden | linkding shown | Lobsters hidden | Lobsters shown |
|---|---|---|---|---|
| Domains recall (judged) | 94% (83-100) | 100% | 90% (86-100) | 81% (71-86) |
| Domains precision (judged) | 93% (80-100) | 96% (89-100) | 72% (57-80) | 81% (67-100) |
| Features recall (judged) | 90% (88-92) | 82% (79-88) | 96% (94-100) | 99% (97-100) |
| Features precision (judged) | 57% (51-65) | 56% (48-67) | 47% (42-49) | 47% (44-51) |
| Narratives in business language (judge) | 82% | 84% | 82% | 80% |
| Use cases fully in business language | 65% (53-81) | 53% (48-63) | 78% | 77% |
| Domains, features, use cases | 6, 43, 112 | 7, 43, 115 | 9, 81, 208 | 9, 80, 216 |
| Calls, time | 138, 6 min | 159, 7 min | 214, 12 min | 239, 13 min |

Cost about $3.5 for the 12 runs (linkding about $0.2 a run, Lobsters about $0.4). Against the single Lobsters run of
2026-10-28 (confidence off), the calls rose from 126 to 214 (the confidence pass and the narrative check) and the
features stay where they were; the domains precision reads 72% against 82%, but the judge misses domain pairs
(below): with the audited pairs it is 85% (80-89) hidden and 88% (78-100) shown.

Hand audit of the judge on one run per series (`hidden-1`, `shown-1`): of the feature pairs it proposed, 3 of 20,
1 of 25 (linkding), 0 of 33 and 1 of 33 (Lobsters) are wrong; of the right pairs not already in `matches.yaml`, it
missed 0, 1, 2 and 3 features (mod mail, notifications, profile settings) and the messaging domain of both Lobsters
runs. Hand feature precision against the judged one at the run: 49% / 53%, 67% / 67%, 45% / 42%, 46% / 44%: the
judge errs both ways, within 4 points. Hand domain precision on Lobsters: 89% / 78% and 88% / 75%, the judge
understates it. The precision of features stays the weakest figure: about a third of the unpaired features of
Lobsters are technical (test data, development checks, caches, dumps), the rest are real features the reference
leaves out (notifications, statistics). The checked pairs are in `benchmark/<repo>/matches.yaml`, except names too
generic to key on (listed in its header).

Re-scored at dd5cf43 (`issues/done/judge_overstates_feature_precision.md`, scoring version 3, no LLM call): the wrong
pairs of the audit now sit in a `rejected` list of `matches.yaml` and no longer count, and each run counts the
judge's pairs no hand has audited. On the audited runs the judged feature precision equals the hand one (49%, 67%,
45%, 46%) with 0, 0, 3 and 1 feature pairs left unaudited (the pairs right in their run but too generic to key
on their name, neither kept nor rejected); the means move by 1 point at most (Lobsters features 48%
on both series), the linkding domains recall drops to 89% hidden and 94% shown (two pairs to `Integrations`
rejected). On the runs not audited, 5 to 28 feature pairs per run rest on the judge alone (mean 13 and 5 on
linkding, 12 and 15 on Lobsters): a judged figure is as good as the share of its pairs audited, not better.

The inputs of both applications were re-frozen from a `hidden` run at this commit; three replays of the domains pass
on linkding find 5 to 7 domains, judged 100% recall and precision each time (the baseline of 2026-10-23 was lost).

## 10. Why generated features match no reference feature (2026-11-03)

Hand classification of the unmatched features of the audited `hidden-1` runs of the dafea18 baseline (section 9),
no LLM call (`issues/done/feature_precision_on_applications.md`):

| Class | Lobsters (51 of 92) | linkding (22 of 43) |
|---|---|---|
| Technical: development, tests, deployment, operations, internal plumbing (test data, dev checks, caches, short ids, health, helpers) | 32 | 12 |
| Too fine: a part of a reference feature (notifications of a message, two-factor setup of the account, favicons of a saved bookmark) | 10 | 9 |
| Right, the judge missed the pair, or a real feature the reference leaves out | 7 | 1 |
| Back-office the reference leaves out | 1 | 0 |
| Wrong | 1 | 0 |

The technical ones dominate: almost every feature with no HTTP route among its entry points is unmatched (33 of 37 on
Lobsters, 10 of 13 on linkding), and the features with a route reach 67% and 60%. The `back_office` mark caught 30 of
the 32 technical ones of Lobsters but 4 of 12 on linkding, and back-office features stay in the docs, listed apart.
Ceilings, against the hand precision of 45% and 49%: the technical ones left out, 68% on both; the too fine ones also
merged into their neighbour, about 82% and 95%.

Change measured (`issues/done/feature_precision_on_applications.md`): the features answer marks a feature `technical`
(development, tests, deployment, maintenance or the internal plumbing, but not an exchange with an external system or
a user nor a copy of what a user saved); such a feature leaves the docs for `features-set-aside.yaml` and the debt
report. Replays x3 of the features pass on the frozen inputs of dafea18, judged figures (unaudited, to compare
settings):

| Setting | linkding recall / precision | Lobsters recall / precision |
|---|---|---|
| Baseline | 90% / 50% (49-51) | 98% / 42% (41-43) |
| T: `technical`, first wording | 94% / 55% (51-61) | 88% / 54% (50-58) |
| T2: external exchanges and saved copies not technical | 93% / 53% (51-55) | 91% / 47% (44-51) |
| M: T2 and "keep a capability whole" (not kept) | 97% / 54% (51-58) | 96% / 51% (48-58) |
| T3: T2 and operations commands and jobs technical (kept) | 89% / 58% (57-60) | 95% / 55% (53-57) |

The first wording set aside real features (page snapshots, webmentions, Mastodon posting); the merge rule left the
too fine features as they were. Whole benchmark of T3 at f95ada6, 3 runs, docs hidden, confidence on, the baseline
re-scored with the same `matches.yaml`:

| Figure | linkding dafea18 | linkding f95ada6 | Lobsters dafea18 | Lobsters f95ada6 |
|---|---|---|---|---|
| Features recall (judged) | 90% | 89% (83-92) | 96% | 98% (97-100) |
| Features precision (judged) | 57% | 63% (59-66) | 52% | 65% (56-77) |
| Features precision by hand, `hidden-1` | 49% | 68% | 45% | 78% |
| Domains precision (judged) | 100% | 75% (62-88) | 85% | 64% (50-78) |
| Features, use cases | 43, 112 | 34, 102 | 81, 208 | 66, 177 |
| Calls, time | 138, 6 min | 121, 6 min | 214, 12 min | 183, 10 min |

The unpaired features left by hand: on linkding too fine ones (asset API, custom CSS, integration settings, unused
tag cleanup) and two technical (toasts, landing redirect); on Lobsters too fine ones (hats, avatars, upvoted list),
real features the reference leaves out (statistics, information pages, moderator list) and three technical (flag
statistics, mail routing, job dashboard). Fewer features also mean fewer use cases and calls. The domain precision
fell with the domains pass unchanged since dafea18 (linkding 8 domains each run against 6, one of them with no
feature): not explained by this change, which runs after the domains.

Fall of the domain precision explained (`issues/done/domain_precision_after_technical_set_aside.md`, no LLM call): it
is mostly the judge's offer, not the domains pass. The judge is offered only the reference domains no generated
domain covers (BSC-4), and `matches.yaml` gained domain pairs after the dafea18 runs were judged, so a second
domain of an already covered reference domain (linkding `Administration`, `Appearance and Integrations`; Lobsters
private messages, hats, membership) could not be paired at f95ada6, while the dafea18 re-scores keep the pairs
their judge made on a fuller offer. The rest are the domains with no feature (linkding `feeds`, `import-export`:
named for entities alone, no resource, nothing set aside) and the Lobsters site operations, unpaired at dafea18
too. With every domain pair of the hidden runs of both commits audited into `matches.yaml` (`--saved-judge`; 0 to 2
pairs left unaudited a run, those of the catch-all linkding `Bookmark Management`):

| Figure | linkding dafea18 | linkding f95ada6 | Lobsters dafea18 | Lobsters f95ada6 |
|---|---|---|---|---|
| Domains precision | 95% (86-100) | 96% (88-100) | 88% (86-90) | 86% (82-89) |
| Domains recall | 83% | 100% | 86% | 86% |
| Domains | 6 (6-7) | 8 | 9 (7-11) | 9 (8-11) |

The 8 domains of linkding sit in the range of dafea18 (5 to 9 over both series). Up to scoring version 4 the
judge's offer stayed as it was, so a judged domain precision fell as `matches.yaml` grew: audit before reading a
fall into a figure of that version (version 5 adds a second call of the judge, below).

Since then (scoring version 4) the docs leave out a domain with no feature and the benchmark no longer counts it
(`specs/featureless-domains.md`, BSC-20). Same audited runs re-scored: linkding 8 to 7 domains in two runs of three
(`feeds`, `import-export`), domains precision 100% (all three runs) against 95% at dafea18; Lobsters unchanged
(86% against 88%: no domain without a feature).

Second pairs the judge's offer hides, read by hand on 2026-11-04 (`issues/done/judge_offer_hides_second_pairs.md`, no LLM
call): the generated features left unpaired by the saved judgements of the 12 hidden runs at scoring version 4, each
classed as a second pair (its right reference feature is already covered, so the judge was not offered it: a part of
it or a second name for it), technical, or other (a real feature the reference leaves out, or a reference feature the
judge was offered and missed). The judge's prompt pairs a dedicated part with its reference feature when it is offered,
as on the fuller offers of the dafea18 runs.

| Figure | linkding dafea18 | linkding f95ada6 | Lobsters dafea18 | Lobsters f95ada6 |
|---|---|---|---|---|
| Unpaired features | 50 | 30 | 110 | 56 |
| Second pairs | 19 | 23 | 42 | 40 |
| Technical | 31 | 7 | 56 | 11 |
| Other | 0 | 0 | 12 | 5 |
| Features precision (judged) | 61% (56-70) | 71% (65-74) | 55% (50-59) | 72% (65-81) |
| The same, second pairs paired | 76% (73-79) | 93% (92-94) | 72% (67-80) | 92% (90-94) |

The bias is 14 to 22 points of judged feature precision and grows as the run sets the technical features aside: at
f95ada6 the second pairs are most of what stays unpaired (profile settings, custom CSS, OpenSearch and REST API parts on
linkding; hats, avatars, notifications, moderator mail on Lobsters). Paired, the judged precision no longer counts a
too fine feature against the run; it reads as the share of the features that are neither technical nor wrong.

Second call of the judge measured (scoring version 5, `benchmark --saved-judge --second-call` on the same 12 hidden
clones, deepseek-chat, about 40 calls; the pairs of the first call kept, so the recalls do not move):

| Figure | linkding dafea18 | linkding f95ada6 | Lobsters dafea18 | Lobsters f95ada6 |
|---|---|---|---|---|
| Features precision (judged), version 4 | 61% (56-70) | 71% (65-74) | 55% (50-59) | 72% (65-81) |
| Features precision (judged), version 5 | 81% (73-85) | 92% (89-95) | 75% (71-77) | 91% (84-100) |
| By hand, second pairs paired (above) | 76% | 93% | 72% | 92% |
| Feature pairs not audited, version 5 | 24 (16-36) | 13 (7-17) | 29 (27-33) | 27 (17-32) |

The judge pairs what the hand classing called second pairs, a little more on dafea18; the domains do not move (their
pairs were audited, the second call proposes nothing for the unpaired ones). The second call's pairs are not audited:
read the unaudited count next to the judged precision. The version 5 baselines are in
`~/retrodoc-runs/judge_offer_hides_second_pairs/<repo>-v5/`.

Set-aside lists read by hand on 2026-11-04 (`issues/done/technical_feature_put_back.md`, no LLM call): the 6 hidden runs
of f95ada6 set aside 83 features (linkding 10, 11, 11; Lobsters 11, 29, 11) and none is a reference feature the docs
lose. They are helpers, route files, operations scripts and jobs, test data and caches; the closest to a user
feature (the linkding HTML snapshot worker, the hat and invitation routes, the story card job, an import helper) are
parts of a feature the same run keeps in `features.yaml`, its entry points left out of it. Operator tasks the
references leave out (Lobsters database dump, privacy wipe, Mastodon list sync) were not judged. No key to put a
technical feature back is needed on the benchmark repositories at this code.
