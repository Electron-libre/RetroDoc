# Stop the entry points pass from listing internal helpers as entry points

# Goal

Now that the entry points pass also reads the business files (`issues/done/business_files_for_entry_points_and_actors.md`),
the files of a web application's service layer, queries and forms reach a prompt written for routes, jobs
and the public API of a library. The model answers with `public_api` or `other` entries for internal helper
functions. Entry points are the base of the domains and features of `issues/behaviour_coverage_domains.md`,
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
   already calls. Needs the references between files (`issues/tree_sitter_structure.md`), so only if 1 and 2
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
2. [ ] Prompt (candidate 1 only): test that locks the wording, change of `ENTRY_POINTS_SYSTEM_PROMPT`, then 3
   paired local runs per repository on the same inputs as before (`roles` and business list kept, only the pass
   replayed). Success: fewer entries outside the reference on linkding without fewer groups or reference files
   read, and the public methods of `delivery_router` still found.
3. [ ] Only if 2 is not enough: bring candidates 2 and 3 back to the user (not planned now).

## Decisions

* Start with the prompt alone: filtering by role is left out until the prompt is measured, since it would
  lose the real background tasks (`services/tasks.py`).
* The measure replays the pass on the inputs of the previous paired runs, locally.
* No paid call without asking.

## Baseline (2026-10-09, linkding reference completed with `services/tasks.py` and the 8 management commands: 25 files)

Replay (`entry_points_replay`) of the 3 paired runs of `issues/done/business_files_for_entry_points_and_actors.md`:

| Pass | Entries per run | Groups found | Entries in reference files | Reference files read |
|---|---|---|---|---|
| role only (before) | 78 (25-110) | 79% (64-86%) | 87% (75-100%) | 69% (36-88%) |
| role + business files (now) | 136 (111-168) | 86% (79-93%) | 65% (50-81%) | 80% (56-92%) |

So the noise to bring down is the 35% of entries outside the reference files (21, 45 and 84 entries in the three
runs), against 13% before the business files joined the pass.
