# Find out why parts of a long entry point file can give no entry point, with no warning

# Goal
A file over `MAX_ENTRY_FILE_CHARS` is read in parts, and the entry points of its parts are merged. In a saved lobsters
run, two of the three parts of the routes file gave no entry point and nothing in the log said so: the routes they
declare are missing from the inventory. Find where they were lost, then make such a loss visible (or impossible).

# Findings (lobsters, deepseek-chat, saved run `second_application_in_benchmark-d8d8c92-1` and `frozen/lobsters`, read on 2026-10-29 at 94a83b9, no LLM call)
* `config/routes.rb` (290 lines, 11.6k characters, 3 parts of about 5,000) has 39 entries, all from part 1: the last
  one, `GET /comments/page/:page`, is at line 126 (character 4,721). Parts 2 and 3 (lines about 132 to 290:
  `resources :messages`, users, settings, invitations, hats, the `mod` namespace) gave none.
* The log has no warning for that pass: no `attributed to an unknown file, dropped`, no `unusable answer for a batch`.
  The last batch is `entry points: 14/14 (92%) 2 file(s), from config/routes.rb`.
* The `surface` run of the same file at 7e2bf9a (same prompt) gave 53 entries covering the whole file.
* The raw answer is not saved: whether the model answered nothing for the parts or the code dropped them is unknown.

# Approach
1. Measure first: replay the entry points pass on `config/routes.rb` alone a few times (frozen lobsters, the file's
   entry removed from the cache: the per-file hash ignores the prompt), logging the items each part answers.
2. Candidates (can be combined):
   1. warn when a part of a file gives no item while another part of the same file does;
   2. never put two parts of one file in the same batch, so each part is asked on its own;
   3. reword the per-file cap of the prompt (`At most 30 entry points per file`) to a per-part one.

# Resources
* `crates/retrodoc-pipeline/src/batched_read.rs` (`BatchedRead::run`, `attribute`: a group with one path takes
  every item, whatever its `file`)
* `crates/retrodoc-pipeline/src/entry_points/mod.rs` (`ENTRY_POINTS_SYSTEM_PROMPT`, `MAX_ENTRY_FILE_CHARS`)
* `issues/routes_file_entries_per_route.md` (the measure that found it)
* The measures ledger (see `CLAUDE.md`)

# Hints
* A single part whose answer is unusable is skipped with no warning (`group.len() == 1`), and the file then never
  gets its reading: not what happened here (the file has entries), but the same loop.
* The joint reading of the business files (`business_read/`) uses the same loop: a fix there covers both.
