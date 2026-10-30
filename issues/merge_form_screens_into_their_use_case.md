# Merge the screens that open a form with the use case that submits it

# Goal
The functional docs of a large application list the same capability twice: a use case "open the creation form" and
another that creates. They must read as one use case, so the number of use cases follows what the business does and
the "open / show / view / edit" screens stop being a fifth of the list.

# Findings (6 contexts, hosted `deepseek-chat`, before and after a prompt that asks for the merge)
* 34 of 285 use cases (12%) of the first real run on a large Rails application start with open, show, view, edit,
  display or new.
* After `issues/done/technical_and_back_office_use_cases.md` asked, in the use cases prompt, for one use case for a route
  that opens a form and the one that submits it, a new run gives 38 of 184 (21%): the prompt alone did not merge them.
  The share rose because the other kinds of use cases dropped, not because the screens did.
* Not measured: how many of the 38 are the first half of a pair (a `new` and a `create`, an `edit` and an `update`) and how
  many are screens that only show data (a `show`, a list) and are legitimate use cases.

# Approach
Measure first, with no LLM: on the saved run, pair the entry points of a resource that open a form (`new`, `edit`, a
`GET` of a form) with the ones that submit it (`create`, `update`, the `POST`/`PATCH` on the same resource) and count the
use cases of such a pair that are two separate ones. Then, candidates (can be combined):
1. Merge mechanically after the use cases pass: a use case that cites only an opening entry point is folded into the use
   case that cites the submitting entry point of the same resource (steps joined, the opening step first, names kept from
   the submit).
2. Group the pair before the use cases prompt: give the use cases pass one feature input per resource action pair, so the
   model sees both entry points and one answer covers them.
3. Keep the prompt only and measure again with more runs, if the pairs are few.

# Milestone
A large application, one bounded context at a time

# Resources
* `issues/done/technical_and_back_office_use_cases.md` (the prompt, the measure `technical_usecases_measure`)
* `crates/retrodoc-pipeline/src/use_cases/` (`prompt.rs`, `grounding.rs`), `entry_points/`
* `.retrodoc/cache/use-cases.yaml` of a run (confidential, not committed)

# Hints
* The application is confidential: no name, path or code of it in a committed file.
* A `show` or a list is a legitimate use case; only the opening of a form that has a submit is a duplicate.
* Do not rely on Rails action names alone: pair by the resource and the HTTP verb as the entry points name them.

# Tracking
1. [x] No-LLM measure `form_pairs_measure` (example): opening entry points paired with a submitting one by `resource` and `outputs`, three cases counted on a saved run; stop and decide with the numbers.
2. [ ] (on hold) The chosen candidate (mechanical merge, pair input before the prompt, or nothing), decided after the measure.
3. [ ] (on hold) Measure again on a complete run (a saved one exists, see "On hold"), update the docs, close.

## Decisions
* Reference run: the latest saved run of the runs directory made after `ab34246` (use cases cite their entry points).
* The pairing must not depend on Rails: an opening entry point is an `http_route` whose `outputs` are only `response`, on a `resource` that also has an entry point with a `db_write`, `email`, `event` or `external_call` output.
* Three cases are counted: an opening split from its submit (candidate), a showing entry point on a resource with a submit (ambiguous, sampled by hand), a showing entry point on a resource without one (legitimate).
* Threshold (user: "see if it is worth it"): above 5% of the use cases list, do the mechanical merge; otherwise the prompt stays as is.

## Measure (step 1, 2026-10-15, the only saved run with cited entry points: 52 use cases, 458 entry points)
* Showing use cases on a resource another use case changes ("paired"): 10 (19%); showing a resource nothing changes: 5 (10%); acting: 33; no known entry point: 4.
* Read by name, only 2 or 3 of the 10 paired open a form; the others are consultations (legitimate). Neither the name nor the text of the entry point separates them well (2 and 3 of 10).
* So the real pairs are about 2 to 5% of the list, around the 5% threshold: neither the mechanical merge nor the pair input is worth it on this run.

## On hold (user, 2026-10-15)
Ledger (2026-10-27): a complete run of the same application is already saved (762 use cases, 750 of them cite
their entry points, listed in the planned measures of the ledger, `CLAUDE.md`), so step 3 is a `form_pairs_measure`
on it, with no new LLM call; this replaces the new run asked for below.
The run is too small (the issue's 184 use cases are not saved). Take it again with a complete run, which costs LLM calls: if the pairs stay under 5%, close with this measure (candidate 3); otherwise the mechanical merge needs a rule that tells an opening from a consultation.
