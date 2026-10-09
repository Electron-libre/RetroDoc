# Use the business files in the entry points and actors passes

# Goal

The business-files pass (ADR 0025) now tells the glossary where the business lives. Two other passes still
choose their input by the old guess: the entry points read the files the roles pass classes `entrypoint`, and
the actors read the files whose name says "authorization" plus the glossary. Check whether they should read the
business list too, or whether the list should also say where the entry points and the authorization rules are,
so that a stack with an unusual layout is handled by the same pass.

# Findings (measures of 2026-10-08 on `delivery_router` and linkding, local Ollama, `qwen3.6:35b-a3b`)

* The `entrypoint` role has the same flaw as `model` had: on `delivery_router`, the whole `lib/**/*.rb` tree
  was classed `entrypoint` in 2 of 5 roles runs (the public API of a library) and a single file in the others.
* Nothing has been measured yet on the entry points and the actors with the business list: they were left
  unchanged on purpose in `issues/done/locate_business_files.md` (decision of step 2, "revisit only if the
  measure shows a gap").
* The business-files pass still lists views, serializers and API routes of a Django application in most
  runs (precision about 0.5 against a hand-made list); a second consumer would inherit that noise.

# Approach

First measure: on `delivery_router` and linkding, list by hand the entry points (routes, commands, public API)
and the actors, then compare with what `retrodoc entry-points` and `retrodoc actors` find over five runs.

Then, candidates (can be combined):

1. The entry points also read the business list, next to the `entrypoint` files.
2. The business-files pass answers a second list (where the entry points are), which replaces the `entrypoint`
   role as the input of the entry points pass.
3. The actors read the business list in addition to the authorization files.
4. Keep things as they are if the measure shows no gap.

# Resources

* `issues/done/locate_business_files.md`, ADR `0025`
* `crates/retrodoc-pipeline/src/business_files/`, `entry_points/`, `actors.rs`
* ADR `0020` (cover behaviours, not files)

# Hints

* The model answers differently each time: judge on several runs and on repositories of different shapes.
* Don't edit `roles.yaml` or `business-files.yaml` by hand to make a run pass (smoke test rule).
* Do not add a call per file.

# Tracking

1. [x] Hand-made reference: the real entry points and actors of `delivery_router` and linkding, read from
   the code and checked by the user (`benchmark/<repo>/entry-points.yaml`, committed: both repositories are public).
2. [x] Offline comparison of the saved runs (no LLM call): a cargo example (`entry_points_replay`, to reuse
   `RoleRules::classify` as it is) reads `entry-points.yaml`,
   `actors.yaml` and `roles.yaml` of the runs kept in the temp directory, and reports recall, precision, count and spread
   per repository and per model, plus the files read; for the runs with a `business-files.yaml`, what
   the business list would add or miss. Table and decision in this issue: keep as is, or go on to 3.
3. [x] Try the simplest candidate (done; kept for every stack, see Decisions) that answers the gap found (only if 2 shows one): test with the fake
   provider, code, docs, then 3 local Ollama runs per repository against the reference.

## Decisions

* The old runs are the baseline: no new LLM call for the measure.
* The reference is written by the agent from the code and validated by the user.
* Results are shown per model, never mixed.
* No paid call without asking.
* The tool is a cargo example of `retrodoc-pipeline`, not a `rust-script`: it applies the real `RoleRules::classify`
  and `BusinessMap::files` to the real tree instead of a copy of them.
* Candidate 1 is kept for every stack, with no threshold on the `entrypoint` role (user, after the paired runs).
* The references are committed under `benchmark/<repo>/entry-points.yaml` (public repositories).

## Offline measurement (2026-10-09, 63 saved runs of `delivery_router`, 31 of linkding, no LLM call)

`cargo run -q -p retrodoc-pipeline --example entry_points_replay -- benchmark/<repo>/entry-points.yaml <checkout at the
benchmark commit> /tmp`. The input tables apply the saved `roles.yaml` / `business-files.yaml` to the real tree; the
pass tables read the saved `entry-points.yaml` / `actors.yaml`. Values are mean (min-max). "Reference files" are the
files where the entry points live (5 for `delivery_router`, 16 for linkding), "outside" the share of the files read
that are not in that list. Samples of the same model are put together, whatever the pipeline version.

`delivery_router` (library):

| Model | Input to the entry points pass | Samples | Reference files read | Files read |
|---|---|---|---|---|
| deepseek-chat | `entrypoint` role | 19 | 31% (0-100%) | 2.3 (0-9) |
| local qwen3.6 | `entrypoint` role | 29 | 29% (0-100%) | 2.3 (0-9) |
| local qwen3.6 | business list | 8 | 82% (80-100%) | 7.8 (5-9) |
| local qwen3.6 | role + business list | 8 | 95% (80-100%) | 8.4 (5-9) |

| Model | Entry points pass, as run | Runs | Entries | Groups found (of 8) | Reference files read |
|---|---|---|---|---|---|
| deepseek-chat | | 19 | 4.5 (0-23) | 25% (0-100%) | 31% |
| local qwen3.6 | | 28 | 2.8 (0-21) | 11% (0-62%) | 29% |

* The pass reads on average a third of the files where the five entry points of the gem live, and often none
  (0 entries in a good part of the runs). The three constructors (`Customer.new`, `Restaurant.new`,
  `Rider.new`) are never found by any run.
* The business list reads 82% of them (8 samples, one model), and 95% together with the role.
* Actors: 91% (deepseek) and 82% (local) of the three reference actors, 0.3 to 0.4 actor outside the reference.

linkding (Django):

| Model | Input to the entry points pass | Samples | Reference files read | Files read | Outside |
|---|---|---|---|---|---|
| deepseek-chat | `entrypoint` role | 7 | 89% (81-94%) | 29.9 | 52% |
| local qwen3.6 | `entrypoint` role | 9 | 86% (81-100%) | 29.1 | 52% |
| local qwen3.6 | business list | 1 | 19% | 11 | 73% |
| any (loose files) | business list | 10 | 47% (6-94%) | 29.6 (10-60) | 77% |
| local qwen3.6 | role + business list | 1 | 100% | 49 | 67% |

* Entry points pass: 94% to 98% of the 14 groups found, about 100 to 130 entries.
* Actors: 51% (deepseek) and 40% (local) of the five reference actors; never the administrator (deepseek) or
  the feed reader (local), and 2 actors outside the reference per deepseek run.

Limits: the references are mine (groups of routes, not one line per route) and linkding's `files` leaves out the
management commands and the background tasks, which are entry points too (part of the "outside" share is
legitimate); the samples mix pipeline versions; only one linkding run has both a role and a business list; the
actors pass does not read its files by role, so this measure says nothing on candidate 3 beyond the numbers above.

Reading: the gap is real on a library (candidate 1 and 2 would help: the business list reaches the files the
role misses), and absent on a Django application where the role already reads 86% to 89% of the reference files
and the business list alone is a poor entry point input (19% to 47%). For the actors, the missing ones
(administrator, feed reader) come from code the authorization files do not hold, which the business list would
not add either.

## Candidate 1 measured (2026-10-09, local `qwen3.6:35b-a3b`, 3 paired runs per repository)

`build_entry_points` now reads the business files next to the `entrypoint` ones (a file in both is read once).
Each pair shares one `brief`, `roles --force` and `business-files --force`, then the entry points pass runs with the
binary before (`old`) and after the change (`new`), on a fresh clone at the benchmark commit; same replay tool.

| Repository | Pass | Entries | Groups found | Entries in reference files | Reference files read |
|---|---|---|---|---|---|
| `delivery_router` | old | 0.7 (0-1) | 8% (0-12%) | 67% | 13% (0-20%) |
| `delivery_router` | new | 13.3 (6-21) | 62% (25-100%) | 50% | 93% (80-100%) |
| linkding | old | 78 (25-110) | 79% (64-86%) | 71% | 58% (6-88%) |
| linkding | new | 136 (111-168) | 86% (79-93%) | 55% | 71% (31-94%) |

* `delivery_router`: the old pass read 0 or 1 file per run (the role classed almost nothing `entrypoint`) and found
  almost nothing; the new one reads the 8 files of the business list and finds 62% of the reference groups.
  `Customer.new`, `Restaurant.new`, `Rider.new` and the four `Router` methods are found in the best runs.
* linkding: the groups found go from 79% to 86%, the reference files read from 58% to 71% (the role of these three
  runs read fewer files than the earlier ones: 6% to 88%). The price: 136 entries instead of 78, and the pass
  takes about twice as long (12 to 13 min against 6 min in the last pair). The entries outside the reference
  files go from 65 to 191: background `job` entries of `services/tasks.py` (a real entry point the reference
  leaves out), but also 40 `public_api` entries for functions of `services/` (`bookmarks.py`, `assets.py`,
  `bundles.py`...) which are internal helpers of the application, not entry points.
* Limits: 3 paired runs, one model, one run of `roles` and `business-files` per pair; the quality of the final
  documents was not measured (no benchmark `generate` run).

Decision of the user: keep the business files in the entry points pass for every stack (no threshold). The
noise of `services/` helpers listed as public API on a web application is left to the prompt of the pass.
