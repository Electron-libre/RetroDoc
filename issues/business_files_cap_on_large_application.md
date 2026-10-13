# Document a large application in several complementary runs, one bounded context at a time

# Goal
A large application must be documented in several runs: a run can stop (budget reached, interruption,
failure) and the next one goes on without redoing or paying again what is done, and each run covers a
vertical slice of the application, a bounded context with its entry points, models, services and rules up
to its features and use cases, not one layer. The docs are usable context by context and grow run after
run. This replaces the fix first asked for here: `BusinessFiles::files` truncates at 60 files after
expanding the located folders, so the entities, the vocabulary and, on the `surface` path, the domains of
a whole application rest on an arbitrary corner of its first folder. With a budget per context that cap
goes away.

# Findings (reading stage of 2026-10-12, a private Rails application, hosted DeepSeek model)
* 1,661 source files. The business files pass named 38 folders and files (models, workflows, services,
  policies), a sensible answer; expanded, they hold about 900 source files.
* `BusinessFiles::files` (`business_files/mod.rs`) expands the entries in rank order, a folder as its
  source files in path order, and truncates at `MAX_FILES = 60`. The 60 files kept are all in the first
  entry: 9 of the 113 files at the root of the models folder (the start of the alphabet) and 51 files of
  three of its sub-folders. The 37 other entries are never read.
* 23 of the 60 files gave an entity. The cache of an older run on the same application, which read the
  files of the `model` role, held 440 model files.
* Cost of that reading: 12 calls, 44,715 tokens in, 3,439 out, 24 s for 60 files, so about 750 tokens in
  per file; the 900 files would be about 0.7 M tokens in.
* The entry points are not capped the same way: `build_entry_points` reads every file of the `entrypoint`
  role plus the business files.
* The cut is silent: the command prints "60 file(s) in all" and no warning says that folders were left out.
* `linkding` (178 source files) and `delivery_router` (9) stay under or near the cap, so no benchmark shows it.

# Approach
Measure first: on `linkding` and on the large application, how many files the located entries expand to,
and how many the cap drops (no LLM call needed: `business-files.yaml` and the file list).
1. Spread the budget over the entries instead of filling it with the first one: a share per entry, by
   rank, so every located folder is read at least in part.
2. Choose inside a folder by something better than path order: the files at the root of the folder before
   its sub-folders, or the ranking of `ranking.rs` (history and references).
3. Make the budget follow the size of the application or a setting, instead of a constant, with the
   expected calls and characters printed before the reading.
4. Report what was left out (count per entry) in the command output and in the report.
5. Ask the business files pass for narrower entries (files, or folders with a pattern) when a folder is
   large.

# Resources
* `crates/retrodoc-pipeline/src/business_files/mod.rs` (`MAX_FILES`, `BusinessFiles::files`), `glossary/`,
  `business_read/`, `entry_points/mod.rs` (`build_entry_points`)
* ADRs `0025` and `0028`; `issues/done/locate_business_files.md`, `issues/done/single_read_per_business_file.md`
* `issues/validate_on_large_application.md` (the reading stage that found it, paused until this is fixed)
* `crates/retrodoc-pipeline/src/ranking.rs`

# Hints
* The application is confidential: no name, path or code of it in a committed file.
* A larger budget is a larger bill on every run: the glossary reading is about 750 tokens in per file.
* The reuse of `business-files.yaml` and of the glossary depends on the list of files: changing the choice
  redoes the reading once.
* Rerunning the reading on the large application costs a few cents on the hosted model; it still needs the
  user's go.

# Tracking

1. [x] Measure without LLM (an example, on the large application and on `linkding`): can the map of the
   contexts be built from the names? Contexts, sizes, share of source files covered, shared files, files in none.
2. [x] No work lost on interruption: one test per pass of the `surface` path (the fake LLM fails midway,
   the rerun redoes only what was missing), passes fixed where needed, a manual kill on `delivery_router`.
3. [x] The map of the contexts: pass, `retrodoc contexts`, `contexts.yaml` (stable, editable, a shared
   file in one context), with its ADR 0029. Measured on the large application with a local and the hosted model.
3b. [x] Orphan files: measured with no LLM call (35% of the support placed by references, a part dubious, see
   "Orphan files placed by references"). Decided (user, 2026-10-12): no persistent placement; the reading
   of a context (step 4) takes the files its entry points reference, so an orphan is read when a behaviour
   reaches it.
3c. [x] Large contexts split: a context of more than 150 files is split by one more call into sub-contexts
   (groups and sub-folders partitioned, nothing lost), since a run is one context. Reason: the central
   context of the application regroups several sub-domains (editing, signing, amending…), user's remark.
4. Reading and documenting per context (plan validated by the user, 2026-10-13). The code showed four couplings
   that would lose the work of earlier runs: the readers (entry points, glossary) rebuild their inventory from
   the files of the run only; the use cases fingerprint holds the global actors and the global top entities;
   the actors are recomputed from every entity; the `surface` domains pick entry points by resource name.
   * 4a. [x] Additive readers: glossary, entry points and the joint reading keep the entries of the files
     outside the run. Test: two readings on two sets of files give the union and the second does not read the
     first again.
   * 4b. [x] The slice of a run, with no LLM call: the contexts (named with `--context`, else the next ones
     not done within a budget of files, a context never cut), the files read for a context (roles entry point,
     model and logic, plus the files its entry points reference at depth 1), and the done state of a context
     (kept in `fingerprints.json` when its whole chain finished).
   * 4c. [x] (library; frozen actors and the wiring are in 4d) Domains, features, use cases and rules per context: `[pipeline] domains_from = "contexts"`, a
     context is a domain with no LLM call, its entry points are those of its files, its vocabulary is its
     own, the actors are computed once then frozen (editable, `retrodoc actors --force`), the artifacts of the
     other contexts are kept as they are. Test: context A then B, A is neither redone nor changed and the
     second run makes no call for A.
   * 4d. [ ] `generate` wiring: `--context`, `--max-files` becomes the reading budget of a run in this mode
     (default 150), the `business-files` pass is skipped, the docs gather the contexts done, the report lists
     the contexts left. Real run on `delivery_router` locally, then on the large application (cost to agree).
5. (merged into 4)
6. [ ] Real validation (the reading stage on one context of the large application, a `surface` benchmark
   on `linkding`; paid runs with the user's go) and the docs.

## Decisions

* (user, 2026-10-12) Two requirements reframe the issue: RetroDoc must run in several times without losing
  work done, and the runs must be complementary along a vertical slice (a bounded context), not a layer.
* (user, 2026-10-12) The slice of a run: named by the user, or the next contexts not done, in the order of
  the map, within a budget.
* (user, 2026-10-12) A context becomes a domain of the docs on the `surface` path: the map replaces the
  domains pass there (ADR to write; touches `issues/single_pipeline_path.md`).
* (user, 2026-10-12) A file shared by several contexts is read once and belongs to one context; the others
  refer to it.
* (user, 2026-10-12) This issue is widened rather than a second one opened. Out of scope: the `files`
  path, the default of `domains_from`, the paused validation run.
* (user, 2026-10-12, after the measure) The map is built in two steps: mechanical groups from the names, then
  one LLM call that names them, drops the technical ones, merges the pairs and places the files left out.
* Deliverable 2 (2026-10-12): every pass of the `surface` path saves after each answer, artifacts are written
  beside, synced then renamed. Checked by a kill -9 of `generate` on `delivery_router` (local model) after 2 of
  5 use cases: the rerun made 7 calls and the next one none. Not done: the features of the `files` path still
  save on a failure only; the record of domains reached, for a domain answered empty at the moment of a kill
  (the rerun keeps its previous features).
* (user, 2026-10-12, after the first maps) A deliverable 3b is added before the reading depends on the map:
  the orphan files are placed by references, not left to the answer. The hosted model is the reference for
  the map (the local one sent half of the files to the support).
* (user, 2026-10-12) The largest context (472 files) is the hub of the business and holds many different
  sub-domains: it must be split into sub-contexts (3c). Orphans: no persistent placement (3b).
* (user, 2026-10-13) Plan of step 4: `domains_from = "contexts"` is a new explicit value; the actors are frozen after
  the first run; the `business-files` pass is skipped in this mode; `--max-files` is the reading budget of a
  run (a context is never cut), default 150.
* If the measure of deliverable 1 shows a poor split before reading, stop and go back to the user before
  any production code.

## Measure of the map from the names (deliverable 1, 2026-10-12, no LLM)

`cargo run -q -p retrodoc-pipeline --example context_map_measure -- [--names] <run dir>...`: one context per
entry point file name (the words that repeat a directory left out), the other source files joined by their
name or a directory of the same name, then the slices of the files of each context (depth 2, 8 files).
Figures after the review of deliverable 3, which made a generic stem (`index`, `urls`, `mod`…) take the name
of its folder and `statuses` meet `status`.

| Repository | Source files | Entry point files | Contexts | Covered (names) | In several | Size p50 / p90 / max | Covered (+ slices) | In several | Size p50 / p90 / max |
|---|---|---|---|---|---|---|---|---|---|
| the large Rails application | 1,676 | 190 | 140 | 60% | 11% | 3 / 22 / 111 | 70% | 36% | 21 / 92 / 320 |
| `linkding` | 176 | 27 | 26 | 97% | 20% | 1 / 3 / 163 | 98% | 37% | 9 / 16 / 166 |
| `delivery_router` | 9 | 1 | 1 | 100% | 0% | 9 / 9 / 9 | 100% | 0% | 9 / 9 / 9 |

* On the Rails application the names give real vertical slices: most of the 15 largest contexts are named
  after a business object and gather its controllers, models, services and policies (20 to 111 files by
  name). They also give contexts named after a technical word (a namespace, a base class, a verb): 5 of
  the 15 largest. Pairs that are one business context come out as two (an object and its join with another).
* The names alone leave 44% of the source files in no context; with the slices 32%, at the price of a
  third of the files in several contexts and a largest context of 320 files. The files in no context are
  mostly a workflow folder organised by its own vocabulary (154), models (94), view components (76) and
  migrations (55).
* On `linkding` (Django: one package named after the main object, files named after their layer) the
  names say nothing: 163 of the 176 files join one context through the name of the package.
* A library with one entry point file is one context.
* So the names are good evidence on a stack that names files after the business object, and not a map by
  themselves: the technical names must be dropped, the pairs merged, the layer-named stacks handled, and
  the files in no context placed. That is the case for one LLM call over the mechanical groups (names,
  sizes, sample of file names, the brief), with the files following mechanically, as the domains of the
  `surface` path are built today from resources.

## First map of the large application (deliverable 3, 2026-10-12, local `qwen3.6:35b-a3b`, one call each)

`retrodoc contexts` on the 1,676 source files, 190 entry point files, 140 mechanical groups (7.4k to 8.6k
tokens in, 1.2k to 2.5k out, 1 to 1.5 min):

| Prompt | Contexts | Largest context | Files in the support |
|---|---|---|---|
| first | 21 (6 merged by the model, 15 forgotten groups kept as contexts) | 444 files | 731 (44%) |
| support limited to what no context uses, big layer folders listed by sub-folder | 19 (4 forgotten groups) | 403 files | 871 (52%) |

* The merged contexts read as business areas (contracts, companies, users and identity, notifications and
  documents, administration). The model also names the leftovers of the mechanical step as contexts, which a
  person can merge.
* The support is too large in both: the model sends whole layer folders (workflows, models, services,
  components, jobs) and about 60 to 87 groups, most of them one job or one endpoint of a business operation,
  to the support, against the instruction. The tighter prompt did not help; this is one local model without
  reasoning and two samples, not a verdict on the design.
* The same call on the hosted model (`deepseek-chat`, 9.6k tokens in, 1.9k out, 10 s, about $0.004): 18
  contexts that read as business areas (contracts, companies, users, documents, notifications, worksites,
  signatures, settings…), the largest 472 files, and 533 files (32%) in the support, which is mostly what it
  should be (migrations, seeds, assets, UI components, helpers, concerns, serializers) plus orphan files of
  policies, presenters, forms and facades that no name reaches. It cites 112 groups twice (the first
  placement is kept). So the weak link of the local runs was the model, not the design.
* Still to do before the reading depends on the map: place the orphans without the answer (a file
  referenced by the files of one context joins it, with the slices of `slices.rs`), and look at the 472-file
  context with the user, who knows whether it is one context.

## Orphan files placed by references (deliverable 3b, 2026-10-12, no LLM)

`cargo run -q -p retrodoc-pipeline --example orphan_placement_measure -- [--names] <run dir>`: a file in the
support joins the context whose files reference it or are referenced by it (`CodeIndex::slice`, depth 1, 30
files at most per file), when the best score is at least 2 and at least twice the second.

| Map | Source files | In the support | Placed | Ambiguous | Weak (score 1) | No reference |
|---|---|---|---|---|---|---|
| hosted map of the large application | 1,676 | 539 | 187 (35%), 155 of a business role | 159 (29%) | 150 (28%) | 43 (8%) |

* About a tenth of the application (155 files) could move from the support to a context, and the sample is
  mixed: jobs, services and state-machine actions land where a reader would put them, but base classes,
  concerns, a utility (an identifier generator), a front-end controller and migrations land in whichever
  context mentions them most. Migrations and data scripts (36 of the 187) are history, not behaviour.
* The files the references reach are, by construction, the files the code slices of the use cases pass
  reads from an entry point of a context (`slices.rs`, depth 2): the behaviour of a context is not cut by
  the orphans being in the support. They matter for the entities of the glossary (the models no entry point
  name reaches), not for the use cases.

## Split of the large contexts (deliverable 3c, 2026-10-12, local model, mechanics only)

`retrodoc contexts --force` on the large application with the local `qwen3.6:35b-a3b`: 2 calls (the map, then
the split of the one context over 150 files), 9.8k tokens in, 2.3k out, 1m44s. The central context came out
as 3 parts (the other contexts were under the limit) and no context is over 230 files any more: the
largest are 227, 118, 104, 90 and 84 files. The support is still 726 files (43%), as with this model before:
that is the model's choice for the layer folders, not an effect of the split. The quality of the split, and the
hosted result, are not measured yet (one more hosted call of about $0.01, which needs the user's go).

### Split on the hosted model (2026-10-12, `deepseek-chat`, 5 calls: the map and 4 splits, 17.9k tokens in, 3.5k out, 19 s, about $0.008)

* The central context came out as 7 parts of 13 to 148 files: editing and life cycle of the contracts,
  signing and signatories, templates and form contracts, documents and PDF generation, library and sharing, the
  integration with the signature provider, the workflow engine. That is the split the user described (editing,
  signing, amending…), and no part is over 150 files.
* The model also produced a context called `support` holding technical groups, which was then split into 8
  contexts (framework core, shared UI components, data and schema management…). Fixed after: a context
  called `support` is the support (tested with a fake LLM, not measured again on the hosted model).
* The review of 3c found, and fixed: the forgotten groups of a split became one context each (they stay
  with the first part now), the original folders of the split context are kept as a fallback on the first part,
  a failed split call is an error (the map is saved without the split first, and `--force` asks again), and a
  part no longer takes the slug of a context left whole. Left as they are: one level of splitting (a part can
  still be over 150 files if its folder cannot be divided), and the split calls run one after the other.
