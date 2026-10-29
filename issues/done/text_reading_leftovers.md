# Decode and cut source texts the same way in every pass

# Goal
`issues/done/shared_lossy_read_and_one_pass_truncation.md` routed the whole-file lossy reads of `retrodoc-pipeline`
through `text::read_file_lossy` and cut `truncate_chars` in one walk. Some reads and cuts still differ from it: two
passes decode the same test files differently, one read cuts a file at a byte offset, three helpers still walk a
text twice to cut it, and two warnings drop the cause of an unreadable file. Make them consistent.

# Findings (code review `high` of `issues/done/shared_lossy_read_and_one_pass_truncation.md`, 2026-10-29)
* `business_files/mod.rs` (`test_vocabulary`) reads test files with a strict `std::fs::read_to_string`, while
  `glossary/mod.rs` reads the same files through `read_file_lossy` and feeds them to the same `test_phrases`: a
  test file with one invalid UTF-8 byte is mined by the glossary and silently skipped by the business files pass.
* `rules/input.rs` (`RuleInput::new`) reads the whole file, then decodes its first `MAX_BYTES_PER_FILE` bytes: the
  cut can split a multi-byte character into a U+FFFD inside the words matched against entity names.
* `chars().count()` then `chars().take()` is still hand-written in `diagrams.rs`, `brief/sample.rs` and
  `sources/mod.rs`.
* The warnings of `rules/prompt.rs` and `use_cases/prompt.rs` on an unreadable feature file log the path only,
  not the io error `PipelineError::Read` carries (`actors.rs` logs it).

# Approach
1. Read the test files of `business_files` through `read_file_lossy` (a change of behaviour: the file is mined
   instead of skipped; check whether a spec covers the test vocabulary).
2. Cut `rules/input.rs` at a character boundary (`floor_char_boundary`-like, or by characters), keeping the
   budget; reading only the prefix (`Read::take`) is an option.
3. Make the three cuts call one helper (a marker parameter on `truncate_chars`, or `char_indices().nth`).
4. Log `error = %err` in the two warnings.

# Resources
* `crates/retrodoc-pipeline/src/text.rs`, `business_files/mod.rs`, `glossary/mod.rs`, `rules/input.rs`,
  `diagrams.rs`, `brief/sample.rs`, `sources/mod.rs`, `rules/prompt.rs`, `use_cases/prompt.rs`, `actors.rs`.

# Hints
* Point 2 changes the fingerprint input of the rules pass only if the text changes; a cut at a boundary changes
  the words of a file cut inside a character: a rerun may redo the rules of such a feature.

# Tracking

Plan: validated

## Understanding
Make the remaining reads and cuts of `retrodoc-pipeline` match `text.rs`: business files mine lossy test files like
the glossary; `rules/input.rs` reads a guarded byte prefix cut back to a character start (also gaining the repo guard
it lacks today); every "first n characters" cut goes through one `text.rs` helper; two warnings log the io error. No
spec covers the test vocabulary nor the rules input (bug fixes, no spec). Out of scope: `retrodoc-ingest`, `retrodoc-mcp`.

## Plan
1. [x] Reads: `text::read_file_prefix_lossy(repo_root, relative, max_bytes)` (path guard, `Read::take`, cut back to
   the start of a character, lossy decode) used by `RuleInput::new`; `business_files::test_vocabulary` reads through
   `read_file_lossy`; `rules/prompt.rs` and `use_cases/prompt.rs` warnings log `error = %err`; verified by unit tests
   (prefix cut inside a multi-byte character leaves no U+FFFD, a path outside the repo is refused, a test file with
   invalid UTF-8 is mined by `test_vocabulary`); ~120 lines.
2. [x] One cut: `text::char_prefix(text, max_chars) -> Option<&str>` (the prefix when the text is longer, one
   `char_indices().nth` walk) called by `truncate_chars`, `brief/sample.rs::squeeze`, `diagrams.rs::shortened` and
   the `chars().take().collect()` excerpts of `sources/mod.rs` and `business_files/mod.rs`, plus `first_chars` (moved
   from `reconcile/judge.rs`) for the same cuts found in `roles`, `ranking`, `chunks`, `chunk_check`, `brief/mod.rs`
   and `reconcile/judge.rs`; verified by the existing
   tests of these helpers plus unit tests of `char_prefix` (exact length, one over, multi-byte, zero); ~80 lines.
3. [x] Fixes of the `high` review: the prefix read keeps a character before a stray continuation byte (`utf8_chunks`),
   both guarded reads share `read_inside_repo`, the manifests are read once (`roles::manifest_excerpts`) through
   `read_file_lossy`, the three marked cuts walk once (`without_last_chars`), two needless copies dropped; verified by
   unit tests (Latin-1 byte at the cut, manifest with invalid UTF-8, manifest linking outside the repo).

## Decisions
- Mode: autonomous
- Base commit: a448d704c8c3235eca737a5902c0d36ecfcc3e94

## Summary of deliverable 1
- done: `rules/input.rs` reads a guarded byte prefix cut back to a character start (`text::read_file_prefix_lossy`);
  `business_files` mines lossy test files; the two feature-file warnings log the io error.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | clippy pedantic clean |
| specs | no spec covers the test vocabulary nor the rules input |
| docs | `docs/ARCHITECTURE.md` "Reads stay in the repo" names the prefix read |
| code review | see below |

- red: none (no spec); before the code: `test_vocabulary` gave `[]` instead of `["refunds a paid order"]`, the prefix
  read gave `"abé"` instead of `"ab"`
- Code review: 1 findings (no bug; the lossy decode was copied from `read_file_lossy`: both now call `decode_lossy`)
- not verified: none
- decisions needed: none

## Summary of deliverable 2
- done: `text::char_prefix` (prefix when longer, one bounded walk) and `text::first_chars` replace every hand-written
  `chars().count()` / `chars().take()` cut of the pipeline (12 sites in 11 files).

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | clippy pedantic clean |
| specs | no change (pure refactor) |
| docs | no doc names the cut helpers: nothing to update |
| code review | see below |

- red: none (refactor; the existing tests of the cuts pass before and after)
- Code review: no findings
- not verified: none
- decisions needed: none

## Summary of deliverable 3 (review `high` of base..HEAD)
- done: the 9 findings of the `high` review fixed: prefix cut by `utf8_chunks` (no ASCII lost on Latin-1, no overflow
  of the budget), one guarded reader (`read_inside_repo`), manifests through `read_file_lossy` in one helper (the
  "Reads stay in the repo" line of `docs/ARCHITECTURE.md` is now true), one walk for `squeeze`, `cut_at_word` and
  `cut`, no copy of a test phrase or a missed example before it is kept.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | clippy pedantic clean |
| specs | no spec covers these reads |
| docs | `docs/ARCHITECTURE.md` already states the guarantee the manifests now meet |
| code review | see below |

- red: none (no spec); before the code: the prefix read of `abc\xa9` at 3 bytes gave `"ab"`, a Latin-1 `Gemfile` gave
  no excerpt, a `Gemfile` linking outside the repo was shown
- Code review: no findings
- not verified: none
- decisions needed: none
