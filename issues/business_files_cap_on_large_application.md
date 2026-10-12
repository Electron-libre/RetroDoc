# Read the business files of a large application beyond the first 60

# Goal
On a large application the glossary must come from the business files the pass located, not from the
first 60 of them in path order. Today `BusinessFiles::files` truncates at 60 files after expanding the
located folders, so the entities, the vocabulary and, on the `surface` path, the domains of a whole
application rest on an arbitrary corner of its first folder. The expected outcome: the files read are
chosen on purpose within a budget that follows the size of the application, and what is left out is
counted and reported.

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
