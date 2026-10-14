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

# Resources
* `issues/done/technical_and_back_office_use_cases.md` (the prompt, the measure `technical_usecases_measure`)
* `crates/retrodoc-pipeline/src/use_cases/` (`prompt.rs`, `grounding.rs`), `entry_points/`
* `.retrodoc/cache/use-cases.yaml` of a run (confidential, not committed)

# Hints
* The application is confidential: no name, path or code of it in a committed file.
* A `show` or a list is a legitimate use case; only the opening of a form that has a submit is a duplicate.
* Do not rely on Rails action names alone: pair by the resource and the HTTP verb as the entry points name them.
