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
