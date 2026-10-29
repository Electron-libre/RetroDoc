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
* `issues/done/routes_file_entries_per_route.md` (the measure that found it)
* The measures ledger (see `CLAUDE.md`)

# Hints
* A single part whose answer is unusable is skipped with no warning (`group.len() == 1`), and the file then never
  gets its reading: not what happened here (the file has entries), but the same loop.
* The joint reading of the business files (`business_read/`) uses the same loop: a fix there covers both.

# Tracking

Plan: validated

## Understanding
The code cannot have dropped the parts: in the 39-entry run (`retrodoc-benchmark-lobsters/1793094583`, the source of
`frozen/lobsters`) the three parts were asked in **one** call (`entry points: 8/13 … 3 file(s), from config/routes.rb`),
a group of one path takes every item, and no truncation or parse warning was logged: the model answered for part 1
only. (The `14/14 … 2 file(s)` line of the findings is the `surface` run, 53 entries, parts 2 and 3 asked together.)
Means: ask each part in a call of its own (candidate 2), warn of a part empty while its siblings are not and of a
single unusable part (candidate 1 and the hint), for the three readings; the prompt is left as is (candidate 3 rereads
every file of every user once, ADR 0037) unless the measure after shows the cap matters.

## Plan
1. [x] Measure before: `retrodoc entry-points` 3 times on a copy of the source run with `config/routes.rb` removed from
   `entry-points.yaml` (one call each, deepseek-chat, ~$0.02 in all); entries and last line covered per run, in the
   issue and the ledger. No code.
2. [ ] One part of a file per call, warnings: `batches` cuts before a second part of a path, the run tracks the
   findings per part and warns (BRD-3), a single unusable call warns (BRD-4); verified by fake-LLM tests of
   `batched_read.rs`; makes true BRD-1 to BRD-4 (BRD-2 traced on the existing test).
3. [ ] Measure after: the same 3 runs at the commit of deliverable 2 (3 calls each, ~$0.02); compared with
   deliverable 1, in the issue and the ledger.

## Measure before (deliverable 1, 2df9c6b, deepseek-chat, ~$0.03)
`retrodoc entry-points` on a copy of the source run whose roles keep only `config/routes.rb` as an entry point file
(the business files are read too, 24 calls a run); the three parts of `config/routes.rb` were asked in one call each
time (`24/24 … 3 file(s), from config/routes.rb`), as in the 39-entry run.

| run | entries | entries of parts 2-3 (messages, settings, invitations, hats, users, `mod`) | last routes covered |
|---|---|---|---|
| 1 | 34 | 8 | `mod` namespace, `/cabinet` (line 289) |
| 2 | 43 | 15 | `/cabinet`, `/jobs` engine |
| 3 | 38 | 10 | `/cabinet`, `/jobs` engine |

The loss does not reproduce: the joint call covered the three parts 3 times out of 3. The 39-entry run is a rare
answer of the model (1 of the 4 joint calls seen), not a drop by the code. Asking each part alone still removes the
case where the model answers for one part of a joint call, and the warning makes the rare case visible: deliverable 2
stays as planned. Artifacts: `~/retrodoc-runs/entry_points_silent_empty_parts-2df9c6b/` (`before-<n>.log`, `.yaml`).

## Decisions
- Mode: autonomous
- Base commit: 2df9c6bf11ecf53056db31483db9f2937a7b7c8a
- Measures: the two paid DeepSeek measures are approved (~$0.04 in all).
- The empty-part warning covers the three readings (glossary, entry points, joint reading).
- The prompt cap is not reworded unless the measure after shows it matters.
