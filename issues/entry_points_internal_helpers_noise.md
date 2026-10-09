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
