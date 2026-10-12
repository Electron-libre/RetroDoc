# Stop the entry points pass from listing internal helpers as entry points

# Goal

Now that the entry points pass also reads the business files (`issues/done/business_files_for_entry_points_and_actors.md`),
the files of a web application's service layer, queries and forms reach a prompt written for routes, jobs
and the public API of a library. The model answers with `public_api` or `other` entries for internal helper
functions. Entry points are the base of the domains and features of `issues/done/behaviour_coverage_domains.md`,
so each helper listed becomes a use case candidate that no user can trigger. Make the pass keep what a user,
a scheduler or another system can trigger, and leave out the internals, without losing the public API of a
library (the reason the business files were added).

# Findings (3 paired runs of 2026-10-09 on linkding, local `qwen3.6:35b-a3b`, after the change)

* 136 entries per run on average against 78 before. 191 entries over the 3 runs are outside the files where
  the hand-made reference puts the entry points (`benchmark/linkding/entry-points.yaml`).
* By origin, over the 3 runs: `services/` gives 48 `job`, 31 `public_api` and 22 `other`; `queries.py` 13
  `other`; `forms.py` 12 `other`; `api/` 9 `public_api` (serializers); `models.py` 6 `other`; the management
  commands 24 `cli_command` (real entry points, also read before the change).
* The `job` entries of `services/tasks.py` are real background tasks that the reference leaves out; the
  `public_api` entries of `services/bookmarks.py`, `assets.py` or `bundles.py` are functions the views call.
* The prompt says "or, for a library, its public API" and "Skip purely technical helpers"; it does not say
  that a service called by a view is not an entry point of the application.
* On `delivery_router`, a library, the same files give the right entries (the gem's public methods): the fix
  must not remove them.
* Not measured: how the extra entries change the domains and features, no `generate` run was made.

# Approach

First measure: complete the reference of linkding with the background tasks and the management commands, so
that "outside the reference" counts only the noise, and keep the figure of the 3 paired runs as the baseline
(`entry_points_replay`).

Then, candidates (can be combined):

1. Tell the prompt what an entry point is for an application (reached from outside: a route, a command, a
   scheduled or queued task, a consumer, a webhook) and that a function only called by other code of the
   application is not one; for a library, say that its public API is, as now. The stack and the product
   brief are already in the header and tell which case applies.
2. Do not send the business files that the role classes as `logic`, `model` or `view` to the entry points
   prompt, only the others; check this does not remove the gem's files on `delivery_router`.
3. Filter after the answer: a `public_api` entry from a file that a route or job entry of the same run
   already calls. Needs the references between files (`issues/done/tree_sitter_structure.md`), so only if 1 and 2
   are not enough.

# Resources

* `crates/retrodoc-pipeline/src/entry_points/mod.rs` (`ENTRY_POINTS_SYSTEM_PROMPT`, `build_entry_points`)
* `crates/retrodoc-pipeline/examples/entry_points_replay.rs`, `benchmark/linkding/entry-points.yaml`,
  `benchmark/delivery_router/entry-points.yaml`
* `issues/done/business_files_for_entry_points_and_actors.md` (the measures), ADR `0008`, ADR `0020`

# Hints

* The model answers differently each time: judge on several runs, paired as in the earlier measure (same
  `roles` and `business-files`, one pass with and one without the change).
* Do not edit `roles.yaml` or `business-files.yaml` by hand to make a run pass.
* Do not add a call per file.

# Tracking

1. [x] Complete the linkding reference with the background tasks and the management commands, replay the 3
   runs made after the business files joined the pass, and note the share of entries outside the reference
   as the baseline (no LLM call).
2. [x] Prompt (candidate 1 only; done, kept): test that locks the wording, change of `ENTRY_POINTS_SYSTEM_PROMPT`, then 3
   paired local runs per repository on the same inputs as before (`roles` and business list kept, only the pass
   replayed). Success: fewer entries outside the reference on linkding without fewer groups or reference files
   read, and the public methods of `delivery_router` still found.
3. [-] Candidates 2 and 3 (not done): the prompt is kept as is; revisit after `issues/done/tree_sitter_structure.md`.

## Decisions

* Start with the prompt alone: filtering by role is left out until the prompt is measured, since it would
  lose the real background tasks (`services/tasks.py`).
* The measure replays the pass on the inputs of the previous paired runs, locally.
* No paid call without asking.
* The prompt (candidate 1) is kept (user, after the paired runs); the `other` entries of `services/` and the lost
  authentication group are left for later.

## Baseline (2026-10-09, linkding reference completed with `services/tasks.py` and the 8 management commands: 25 files)

Replay (`entry_points_replay`) of the 3 paired runs of `issues/done/business_files_for_entry_points_and_actors.md`:

| Pass | Entries per run | Groups found | Entries in reference files | Reference files read |
|---|---|---|---|---|
| role only (before) | 78 (25-110) | 79% (64-86%) | 87% (75-100%) | 69% (36-88%) |
| role + business files (now) | 136 (111-168) | 86% (79-93%) | 65% (50-81%) | 80% (56-92%) |

So the noise to bring down is the 35% of entries outside the reference files (21, 45 and 84 entries in the three
runs), against 13% before the business files joined the pass.

## Candidate 1 measured (2026-10-09, local `qwen3.6:35b-a3b`, 3 runs per repository on the same inputs as the baseline)

`ENTRY_POINTS_SYSTEM_PROMPT` now says an entry point is what a user, a scheduler or another system triggers from
outside, that a function only called by other code of the application (service, query, form, serializer, helper)
is not one even when public, and that a library's public API is its entry points. Only the pass was replayed.

| Repository | Pass | Entries | Groups found | Entries in reference files | Reference files read |
|---|---|---|---|---|---|
| linkding | before the prompt | 136 (111-168) | 86% (79-93%) | 65% (50-81%) | 80% (56-92%) |
| linkding | prompt | 107 (92-123) | 79% (71-86%) | 70% (60-80%) | 80% (56-92%) |
| `delivery_router` | before the prompt | 13.3 (6-21) | 62% (25-100%) | 50% | 93% |
| `delivery_router` | prompt | 13.3 (9-16) | 46% (25-62%) | 58% | 93% |

* linkding: 320 entries over the 3 runs instead of 407; those outside the reference files go from 150 to 98
  (37% to 31%, 30% on the per-run mean). The `public_api` entries of `services/` fall from 31 to 8 and the `job`
  entries outside the reference from 32 to 4, but the model now files 60 service entries as `other` (21 before):
  it labels the helpers differently more than it leaves them out.
* linkding, group lost: "login, logout, password" is no longer found. In the run that had them, the two entries
  of `views/auth.py` (login and change password views, thin classes over Django's) are not listed any more: the
  prompt makes the model skip a view with little code of its own. One entry in one run: noise or a real side
  effect, the data cannot tell.
* `delivery_router`: the fall of the groups found (62% to 46%) comes from the names, not from the entries. With
  the prompt, the model often names a method without its class (`add_order` instead of `Router#add_order`), which
  the regexes of the reference do not match. Checked by hand on the same entries: the four `Router` methods and
  `DeliveryRouter.new` are found in 2 of the 3 runs before and 2 of the 3 with the prompt; the three constructors
  in 1 run before and 1 with the prompt. No loss visible, but 3 runs cannot show a small one.
* Limits: 3 runs per repository, one model, a reference of mine; the tool matches names, not meaning (see the
  `delivery_router` item).

Reading: a modest gain on the application (about a fifth fewer entries, `public_api` helpers mostly gone) at the
cost of one group in one run, and nothing clear on the library. The noise is not gone: the `other` entries of
`services/` and the front-end components (`frontend/`, 12 entries) remain.
