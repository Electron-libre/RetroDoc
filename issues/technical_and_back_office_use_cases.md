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
2c. [ ] Align the others on `UseCase::is_technical`: the debt report (`technical_use_cases`), and a marker in the MCP answers and the search results (no change of ranking).
3. [ ] Mechanical check that the human actor of the first step matches a human entry point.
4. [ ] Use cases prompt: system, external provider or API consumer as actor without a human entry point.
5. [ ] Entry points: in an application, a public method is an entry point only if a route, job, workflow or webhook reaches it; else technical support.
6. [ ] Use cases prompt: a form-opening action and its submit are one use case.
7. [ ] Re-validate on the large application (needs the user's agreement before the paid run); figures to the issue and `PLAN.md`, generically.

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
