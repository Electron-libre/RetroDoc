# Decode and cut source texts the same way in every pass

# Goal
`issues/shared_lossy_read_and_one_pass_truncation.md` routed the whole-file lossy reads of `retrodoc-pipeline`
through `text::read_file_lossy` and cut `truncate_chars` in one walk. Some reads and cuts still differ from it: two
passes decode the same test files differently, one read cuts a file at a byte offset, three helpers still walk a
text twice to cut it, and two warnings drop the cause of an unreadable file. Make them consistent.

# Findings (code review `high` of `issues/shared_lossy_read_and_one_pass_truncation.md`, 2026-10-29)
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
