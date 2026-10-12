# Validate the redesigned pipeline on a large real application

# Goal
Know whether RetroDoc now reaches its product goal on its real target: a large legacy application whose
functional documentation a reader who does not know the code can use. Every measure since the redesign
(ADRs 0019 to 0028) comes from two small public repositories; the last complete `generate` on a large
application predates the redesign. One complete run, read by the user, says whether the direction holds
and gives the evidence the parked decision on `domains_from` is waiting for.

# Findings (project review of 2026-10-12)
* The only complete `generate` on a large application is of 2026-10-02 (`PLAN.md` §7.2): a sparse checkout
  of 325 source files of a private Rails application, local `qwen3.6:35b-a3b`, about 2h20, 7 domains, 77
  features, 183 use cases, 13 features without a use case. It ran the `files` path, before the brief, the
  business files, the structured outputs, the mechanical confidence, tree-sitter and the rules.
* On the cache of an older run of that application, 36% of its 1,661 source files are reached by a
  behaviour (`issues/done/behaviour_coverage_domains.md`); nobody has read what the other 64% are.
* The `surface` path is measured on linkding (178 source files, deepseek: 110 calls and 6.4 minutes against
  222-283 and 9.7-14.4) and on a 9-file library. Not on a repository of thousands of files.
* The local model is about ten times slower than the hosted one on linkding (86-118 minutes against
  10-14): the whole application is out of reach locally in one sitting.
* The application is confidential: sending its code to a hosted provider is the user's decision.

# Approach
1. Decide with the user: which application, hosted model or local, whole repository or a bounded part
   (`--max-files`, sparse checkout), `files`, `surface` or both, rules on or off. Estimate the cost first
   from the tokens per source file of the linkding runs (`usage.json`).
2. Run it with the smoke-test skill's criteria (finishes, warnings, rerun is a no-op, report) and keep the
   recap per pass: calls, tokens, time, unparseable answers, skipped units.
3. Read the result by hand with the user, on a sample fixed before the run: domain names, 20 use cases
   picked evenly, the rules of 3 features, the entry points without a feature and the technical support
   list. For each: right, wrong, technical noise, missing.
4. Write the figures and the reading in this issue, generically (no name, path or code of the application),
   and turn each defect into an issue or a line of an existing one.
5. Give the result to `issues/surface_domains_default_decision.md`.

# Resources
* `PLAN.md` §7.2 and §7.5, `issues/done/behaviour_coverage_domains.md`
* `issues/surface_domains_default_decision.md`
* Skill `smoke-test`; `crates/retrodoc-pipeline/examples/behaviour_baseline.rs`
* `CLAUDE.md` (confidential repositories)

# Hints
* Never name the application, nor quote its paths or code, in a committed file.
* A paid run needs an explicit go from the user, with the estimate.
* One run is one sample: it validates the direction and finds defects, it does not compare two settings.

# Tracking

1. [x] Preparation without LLM call: clone in `/tmp`, config, `scan`, reading sample fixed.
2. [ ] The paid run: `generate`, rerun, `report`, verdict of the smoke-test criteria, `behaviour_baseline`;
   figures per pass written here.
3. [ ] Reading with the user on the fixed sample; the reading written here, generically.
4. [ ] Each defect to an issue or a line of an existing one; result given to
   `issues/surface_domains_default_decision.md` and `PLAN.md` §7.6.

## Decisions

* (user, 2026-10-12) The private Rails application of the earlier smoke tests, at its committed `HEAD`:
  1,661 source files, 670 test files (`scan`).
* (user, 2026-10-12) Hosted model (DeepSeek), `surface`, rules off, one run. A ceiling of $15 was refused as
  far too much at this stage; the order of magnitude accepted is $1 to $2.
* The smoke script only targets the local server: the same steps are run by hand and judged with
  `smoke.rs evaluate`.

* (user, 2026-10-12) Paused after the reading stage: the second stage waits for
  `issues/business_files_cap_on_large_application.md`, since a reading made now would be dominated by that defect.
* Not asked for the budget finding below (`--max-files` on the `surface` path): still to turn into an issue
  or a line of one (deliverable 4).

## Reading sample (fixed before the run)

* every domain name;
* 20 use cases: the list sorted by identifier, one every `N / 20`, starting at the first;
* the entry points no feature holds, and the technical support list (the first 30 of each if longer);
* no rules (the pass is off).

## Findings while preparing

* `--max-files` does not bound the `surface` path: `generate` applies the budget after the surface passes
  (roles, business files, glossary, entry points) and the actors, and the features of the `surface` path are
  built from every entry point, kept file or not. The budget only removes files from the code slices the use
  cases read and from the coverage figures. To record as a defect (deliverable 4).

## Reading stage (2026-10-12, hosted model)

Run in two stages to hold the cost: `brief`, `roles`, `business-files` and `business-read` first, the rest of
`generate` after a look at the figures. The second stage was not started.

| Command | Calls | Tokens in | Tokens out | Time |
|---|---|---|---|---|
| `brief` (sources, brief) | 3 | 33,668 | 1,234 | 8 s |
| `roles` | 1 | 13,796 | 1,088 | 5 s |
| `business-files` | 1 | 19,278 | 853 | 5 s |
| `business-read` | 12 | 44,715 | 3,439 | 24 s |
| Total | 17 | 111,457 | 6,614 | 42 s |

About $0.04 at the high prices. No unparseable answer.

* **Blocking: the business read stops at 60 files.** The business files pass located 38 folders and files,
  sensible ones (models, workflows, services, policies), which hold about 900 source files; `BusinessFiles::files`
  expands the folders in rank order and truncates at `MAX_FILES` (60), so the 60 files read are the first 60,
  in path order, of the first folder. The glossary reads the business files only, so the entities of the
  whole application come from those 60 files (3.6% of the 1,661 source files): 9 of the 113 files at the root
  of the models folder, the rest in three of its sub-folders. 23 of the 60 files gave an entity.
* 11 entry points after this stage, none of them a route, which says nothing yet: the standalone
  `business-read` is only the shared reading of the business files. In `generate` the entry points pass then
  reads the files of the `entrypoint` role too (controllers, API, routes, jobs, tasks: more than 129 files
  here), so the second stage would have read them. A first note in this issue said the opposite; it was a
  misreading of the staged commands, corrected after reading `build_entry_points`. The cache of the older
  run held 582 entry points in 167 files.
* 60 entries and no test vocabulary in the glossary after this stage (440 model files in the older cache).
* Warnings: the server refuses the strict JSON schema at each command (the run goes on without it); one
  source rule for the database schema is dropped (its file cannot be read in the expected format); one
  business path the LLM named does not exist; one long file is cut after 13 chunks.
