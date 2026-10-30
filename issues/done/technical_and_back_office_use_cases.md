# Keep technical helpers, back-office tools and system-triggered behaviours out of the business use cases

# Goal
The functional docs of a large application must read as what the business does. The first real run on a large
Rails application (`issues/done/validate_on_large_application.md`, control pass of 2026-10-13) shows that about a
third of the use cases are not that: public methods of application classes read as behaviours, back-office
tools, and behaviours nobody human triggers described with a human actor. Set them apart or leave them out, so
the core of the business is what a reader meets first.

# Findings (6 contexts, 161 use cases, hosted `deepseek-chat`)
* 48 use cases (30%) belong to features made mostly of `public_api` entry points: the public methods of
  application classes (PDF generation, a client of an external company directory, model helpers). Typical
  narrative: "a caller requests the PDF as a binary and no file is written". The entry points pass
  keeps them since the business files are read for entities and entry points (ADR 0028); the earlier fix
  (`issues/done/entry_points_internal_helpers_noise.md`) was measured on a Django application and a library.
* The human actor is named even when nothing human triggers the behaviour: 68 use cases belong to features
  with no HTTP route (workflow steps, jobs, provider callbacks, service classes), 48 of them have the most common
  actor (108 of the 161 use cases have it). One use case invents a user submitting identifiers to a service class
  that no code read calls; 11 use cases have a named actor and a first step done by the system.
* 9 use cases come from controllers of an admin namespace (back-office tools: import and reimport of a form
  definition from a JSON payload); the pipeline flags them already, with a business-language score of 0.7 and a
  confidence of 0.5, but the docs show them like the others. 12 use cases have a business-language score under
  0.8, 32 have code-level wording in its rationale, 21 have a confidence under 0.6.
* 20 use cases (12%) are "open / show / view / edit" screens, several the first half of a pair of Rails actions
  (open the creation form, then create): the same capability twice. Two imports from JSON are near-duplicates.

# Approach
Measure first on the saved run (no LLM): for the use cases flagged by each signal, how many a reader would drop.
1. In an application (not a library), keep a public method of a class as an entry point only when a route, a job,
   a workflow or a webhook reaches it (the references of `slices.rs`), else list it as technical support.
2. In the use cases prompt and the grounding: no human actor unless a human entry point (route, form) leads to the
   behaviour; otherwise the system, the external provider or an API consumer. Check mechanically that the first
   step's actor matches the kind of the entry point.
3. In the docs, put the use cases of an admin namespace and those under thresholds of business language and
   confidence in a separate section ("Administration and technical"), not in the index of the business.
4. Ask the use cases pass to cover a form-opening action and its submit as one use case.

# Resources
* `issues/done/validate_on_large_application.md` (control pass), `issues/done/entry_points_internal_helpers_noise.md`
* `crates/retrodoc-pipeline/src/use_cases/`, `entry_points/`, `vocabulary.rs`, `crates/retrodoc-render`
* ADR 0028; `.retrodoc/cache/use-cases.yaml` of the run (confidential, not committed)

# Hints
* The application is confidential: no name, path or code of it in a committed file.
* Do not remove the public methods of a library (the reason business files feed the entry points pass).

# Tracking
1. [x] Measure on the saved run, no LLM (`examples/technical_usecases_measure.rs`): use cases per signal, and which signal best finds the administration ones.
2. [x] Render a section "Administration and technical" (admin, business language < 0.8 or confidence < 0.6), out of the business index: `Feature.back_office` and `UseCase::is_technical` in the model, the render splits the feature, domain and `llms.txt` pages.
2b. [x] Features pass: the LLM sets `back_office` from the description, the routes and the files (both prompts, files and surface); kept by the reuse and the partial redo; docs fixed. Known: the prompt is not in the fingerprints, so a saved run keeps `false` until `--force`.
2c. [x] Align the others on `UseCase::is_technical`: the debt report counts what is set apart (its own "technical-sounding" list keeps its 50% debt threshold), and a marker in the MCP answers and the search results (no change of ranking). The CLI `search` answers from the same `Docs`, so it carries the marker too.
3. [x] Mechanical check: a use case reached only through entry points no person starts (job, consumer, webhook, public API) has no human actor; its human steps become the calling program, the external provider or the system, and the primary actor is dropped (`without_invented_humans`). The narrative is not rewritten (step 4's prompt). A feature with no entry point is not judged.
4. [x] Use cases prompt: each entry point shows its kind; only a route or a command is started by a person, else the actor is the system, the external provider or the calling program, with no `primary_actor`. Not measured on a real run (step 7).
5. [x] Entry points: in an application, a `public_api` entry is kept only if its file is reached (2 hops of references) from a route, command, job, consumer or webhook entry; else it is left out for the later passes (technical support); a library keeps all (`without_unreached_public_api`). Measure on the saved run: 40 of 106 `public_api` entries left out, 7 of the 17 use cases that cite only `public_api` entries (2% of 285). A workflow has no entry point kind of its own, so it counts only through the route or job that reaches it.
6. [x] Use cases prompt: a route that opens a form and the one that submits it are one use case citing both (entry points addendum). Not measured on a real run (step 7); a mechanical merge was not done.
7. [x] Re-validated on the large application (user's agreement given; figures below and in `PLAN.md` §7.7, generically).

## Decisions
* Set apart, do not delete: the use cases stay in the cache and the render, in their own section.
* Thresholds: business language < 0.8 or confidence < 0.6, plus administration.
* Administration is detected from what the LLM determined (context, role, actor), not from a folder name or a config key; the measure (1) picks the signal.
* Run used for the measure: `/tmp/retrodoc-validate-ctx/.retrodoc/cache/` (confidential, not committed).
* Order: measure, render, actor check, prompts and entry points, then re-validation.

## Measure (step 1, saved run of the large application, 285 use cases, 6 contexts)
* `public_api` feature 21%; human actor with no route nor command 28%; screens 12%; business language < 0.8 6%; confidence < 0.6 13%.
* Administration: the reference (sources in an `admin` folder) is 15%. The primary actor named "administrator" matches it badly (18 in both, 25 actor only, 25 folder only: a customer's company administrator is business, not back-office). The name or description of the feature matches best (26 in both, 6 only, 17 folder only), and still misses back-office features described with a business role.
* Set apart with the feature text and the two thresholds: 27% of the use cases.
* The figures of the Findings come from an earlier pass (161 use cases); the saved run is a later one.

## Re-validation (step 7, 2026-10-14, 8 of 35 contexts, hosted `deepseek-chat`, `generate --force`, one run)
One pass: 191 calls, 936k tokens in, 164k out, 14 minutes. The saved run before had 285 use cases (several passes
added up), the new one 184: another LLM run, so part of the change is spread.

| Signal | Before (285) | After (184) |
|---|---|---|
| feature made mostly of `public_api` entry points | 61 (21%) | 10 (5%) |
| human actor, no route nor command | 80 (28%) | 13 (7%) |
| business language < 0.8 | 18 (6%) | 7 (4%) |
| confidence < 0.6 | 36 (13%) | 14 (8%) |
| open / show / view / edit screens | 34 (12%) | 38 (21%) |
| set apart in the docs | 78 (27%) | 57 (31%) |
| back-office features | 0 | 5 of 52 (35 use cases) |

* The docs have the section "Administration and technical" on 24 pages, in the 3 domain pages and in `llms.txt`.
* 10 steps got the calling program or the external provider as actor, 26 use cases have no primary actor (8 before).
* In an application, 35 of the 57 `public_api` entries of the inventory are left out.
* Not solved: the form screens did not drop; the prompt alone does not merge them. Opened as
  `issues/done/merge_form_screens_into_their_use_case.md`.
* Not checked by hand: the 5 back-office features and their 35 use cases; one run only.
