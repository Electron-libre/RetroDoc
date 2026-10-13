# Give each context the entry points it serves, and cover a context with hundreds of them

# Goal
A bounded context must be documented from all the entry points it serves and only those. The second series of
real runs on a large Rails application (`issues/business_files_cap_on_large_application.md`) shows that the largest
context sees only a third of its entry points in the features call, and that a central routes file gives the
routes of the whole application to one context.

# Findings (2026-10-13, hosted `deepseek-chat`, 35 contexts, the first two runs: 8 contexts, 46 features, 203 use cases)
* The central context (199 files) has 456 entry points in its files. The features call lists 150 of them
  (`MAX_ENTRIES_PER_PROMPT`); 308 were not claimed by any feature, the placement call (batches of 60) placed 77,
  and 231 stayed in `behaviour-coverage.yaml` as held by no feature. Overall 739 entry points were read, 403 are
  held by a feature, 231 are listed as unplaced.
* 131 of the 456 are the routes of the central routes file (`config/routes.rb`), which routes 77 different
  resources of the whole application; 110 of the 231 unplaced entry points are routes of that file. Entry
  points are given to the context that owns their file (`DomainCluster::by_paths`), so the routes of every
  context land in the one that owns the routes file, where nothing describes them.
* The largest controller adds 27 more unplaced entry points; the rest are workflow families and an external
  directory client.
* The features of that context are still good (17 features, 120 use cases, none without a use case): what is
  missing is what the call never saw.

# Approach
Measure first on the saved run (no LLM): for each route of the routes file, the controller it routes to and the
context that owns that controller.
1. Give a route entry to the context that owns the controller it routes to (the entry names the resource or the
   controller; match it to the entry point file by name, as the mechanical groups do), else keep the file's
   context. The same for any file that only declares entry points for other files.
2. When a domain has more entry points than a prompt holds, ask the features in several calls over slices of its
   entry points grouped by file, and merge the features of the same name; keep the one-call path under the limit.
3. Report per context the entry points held by no feature, in the output and in the coverage report.

# Resources
* `crates/retrodoc-pipeline/src/features/from_surface.rs` (`domain_entries`, `MAX_ENTRIES_PER_PROMPT`,
  `place_per_domain`), `domains.rs` of `contexts/`
* `issues/use_cases_record_their_entry_points.md`, `issues/technical_and_back_office_use_cases.md`
* `issues/business_files_cap_on_large_application.md` (the real runs)

# Hints
* A context already documented has its features saved by domain: changing the entries of a domain redoes its
  features once. Warn the user before a rerun that costs.
* The application is confidential: no name, path or code of it in a committed file.

# Tracking

1. [x] Measure without LLM (an example): for each entry point of a file that only holds routes, the controller it
   routes to and the context that owns it; entry points that change context, those with no controller found,
   context sizes before and after, contexts over 150.
2. [x] A route goes to the context that owns the controller it routes to, else stays in the context of its file
   (same for any file that only declares entry points for other files).
3. [x] Several features calls over slices of entry points grouped by file when a domain has more than
   `MAX_ENTRIES_PER_PROMPT`; features of the same slug merged (no LLM); saved after each slice, one print per slice.
4. [x] Entry points held by no feature reported per context, in the output, `behaviour-coverage.yaml` and the report.
5. [ ] Docs (`skill:update-docs`), then a real rerun of the central context on the saved run, after agreeing on
   the cost with the user.

## Decisions

* (user, 2026-10-13) A route with no controller found stays in the context of its file (no pseudo-context).
* (user, 2026-10-13) Slices of at most 150 entry points grouped by file, a file over 150 cut as a last resort;
  features of the same slug merged without an LLM call (entry points united, first description kept).
* (user, 2026-10-13) Saved after each slice, one print per slice.
* The saved run for the measure and the real rerun: `/tmp/retrodoc-validate-ctx` (outside the repo, not named in committed files).

## Measure of the routes by owner (deliverable 1, 2026-10-13, no LLM)

`cargo run -q -p retrodoc-pipeline --example route_owner_measure -- <run dir>...` on the saved run of the
second series (35 contexts, 739 entry points). No framework convention: a route (`http_route`) goes to the
context that owns the entry point file it names, found by the name of that file (`name_words`, the words that
repeat a directory left out) against its resource (whole, then last word), then the static segments of its
path; a name owned by two contexts says nothing and the next name is tried; a route that names a word of its
own file's name stays with it (`EntryOwners`, `contexts/route_owner.rs`). A first version looked for files
ending in `controller`, which is a Rails convention: dropped (user's remark). Without any notion of handler,
moving every kind of entry point sent the actions of the workflow files to the context of the resource they
name (55 to 2 in one context): only routes move.

* 318 routes; 130 move to another context, 3 are already in the right one, 185 stay with their file (they
  name a word of its name, as the routes of a handler file do, or no single context owns the file they name).
* The routes file is in no context (it is read for one): 166 entry points were held by whichever context
  read it. After the move 116 of them are in the contexts they serve (worksites, users, documents, compliance,
  companies… from 0 or 2 entry points to 12 to 22 each) and 50 stay with the file.
* Contexts over the prompt limit (150): the central one stays over it (298 entry points in its own files, 308
  after), so the slices of deliverable 3 are needed.
* A sample of the moved routes read by eye on the first version: they land where a reader would put them.
  Not re-read on this version.
