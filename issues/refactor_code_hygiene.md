# Hygiene, maintainability and readability refactorings

# Goal

Reduce the duplication and oversized files found during a review pass over the whole repo, without changing
any behavior: the existing tests, `cargo clippy` (pedantic) and a second `generate` writing 0 files must stay
green after each deliverable.

# Findings

Health status: `cargo clippy --workspace --all-targets` has no warning, `#[allow]`s are rare and justified
(7, all `cast_precision_loss`/`float_cmp`, commented or local), no `todo!`/`dbg!`, no `println!` outside the
CLI, `unwrap`/`expect` confined to test modules. The foundation is sound; the points below are structural
debt, in decreasing order of interest.

1. **Duplicated batched read loop** (`glossary.rs` ~l.296-345, `entry_points.rs` ~l.225-285). The same
   algorithm copied: batches of files → `--- path ---` prompt → `complete_json` → file-by-file fallback if
   the batch is unusable → per-file attribution → count of remaining chunks → merge → save after each batch.
   Only the response type, the system prompt and the merge differ. `entry_points.rs` also imports `batches`
   from `glossary.rs` (coupling between two sibling passes). Any fix (e.g. the fallback, the partial save)
   currently has to be made twice. The tests are duplicated too (`OnlyOneFileAtATime`, `ScriptedProvider` in
   both).
2. **CLI command preamble copied 7 times** (`glossary`, `actors`, `entry_points`, `roles`, `surface`,
   `scan`, `generate`): `canonicalize` + `Config::load` + `retrodoc_ingest::run` with the same `context(...)`
   messages, and the collection of `source_files` (`FileKind::Source`) which exists in two versions
   (`generate::source_paths` and the inline block of `actors.rs`). The error messages are byte-for-byte
   identical, so easy to diverge.
3. **`.retrodoc/cache/*.{yaml,json}` paths scattered**: one `*_RELATIVE_PATH` constant per module (13), each
   with its `load`/`save` pair of the same shape (`artifact::load_yaml` + `join`), plus literals in the CLI
   messages (`generate.rs`, `glossary.rs`, `actors.rs`, `entry_points.rs`, `roles.rs`) and the file list of
   `clear_caches`. Renaming an artifact means touching all these places, and `clear_caches` can forget an
   artifact without anything signaling it (already true for `scope.yaml`, to check whether intended).
4. **`OpenRouterProvider::complete` is ~112 lines with 5 levels of nesting** (`retrodoc-llm/src/lib.rs`
   l.269-380): request construction, retry loop, response decoding, truncation warning and delay computation
   are mixed. `lib.rs` (793 lines) also groups public types, the `HeartbeatProvider` decorator, the HTTP
   client and tests.
5. **`generate::run` orchestrates 11 passes in a row with interleaved `println!`s and `tracker.set_pass`
   calls** (`generate.rs`, 441 lines of which ~140 are display). Manual `set_pass`/`end_pass` are easy to
   forget (a miss attributes the tokens to the wrong pass); the display (`print_*`) is mixed with the
   orchestration.
6. **`LlmProvider` fakes copied in tests**: 28 implementations (`CannedProvider` ×4, `CountingProvider` ×4,
   `ScriptedProvider` ×3, `FlakyProvider` ×2, `RecordingProvider` ×2…), most of them 10 to 30 lines that only
   differ by the response returned.
7. **Large files mixing types, logic and tests**: `glossary.rs` (759), `confidence.rs` (663),
   `features.rs` (646), `entry_points.rs` (562), `roles.rs` (552), `markdown.rs` (535). The `repo_map/`,
   `domains/` and `use_cases/` modules already have the right split (`mod.rs` + `tests.rs`); it is not
   applied elsewhere, tests making up 40 to 60% of these files.
8. **Unrelated size constants**: about thirty `MAX_*_CHARS` / `MAX_*` local to the passes
   (`MAX_FILE_CHARS` 6000, `MAX_MODEL_FILE_CHARS` 4000, `MAX_ENTRY_FILE_CHARS` 5000, `BATCH_CHARS` 12000…),
   some with `_` separators, others without (`6000`, `4_000`). Not a defect in itself, but the literal style
   is inconsistent and the reason for the values is not always documented.

# Approach

One deliverable per point, in this order, each with its `refactor(...)` commit and no behavior change:

1. **Shared batched read**: extract into the pipeline (a `batched_read.rs` module, for example) a generic
   driver parameterized by the system prompt, the attribution function and the merge; move `batches` there.
   `glossary` and `entry_points` keep only their schema and their merge. Share the corresponding test fakes.
   Check that `a_batch_that_cannot_be_answered_is_retried_file_by_file` and the partial save pass unchanged.
2. **CLI command context**: a `Workspace { repo_root, config }` struct (`commands/context.rs`) with
   `open(path)` (canonicalize + config, same messages) and `ingest()` / `source_files()` methods. The 7
   commands use it; remove `source_paths` and the inline block.
3. **Artifacts**: an `artifacts` module exposing the file names in a single place (enum or constants), used
   by `load`/`save`, by `clear_caches` (list derived rather than copied) and by the CLI messages. Explicitly
   decide whether `scope.yaml`/`roles.yaml`/`usage.json` are kept by `--force` and record it in the
   function's doc.
4. **`OpenRouterProvider::complete`**: extract `build_request`, `decode_success` (choices, truncation,
   usage) and `retry_delay(attempt, server_delay)`; the network error and the retryable status share the
   backoff computation. Split `lib.rs` into `types.rs`, `heartbeat.rs`, `openrouter.rs` keeping the re-exports
   (the public API does not change).
5. **`generate::run`**: extract the display into `commands/generate/report.rs` (or `print.rs`) and
   introduce a small guard `tracker.pass("roles")` that calls `end_pass` on drop, so that non-LLM passes no
   longer steal tokens by omission. While at it, `run` takes 6 parameters: group
   `dry_run/force/confidence/max_files` into a `GenerateOptions`.
6. **Test fakes**: a `testing` module (`#[cfg(test)]`, or a `test-support` feature of `retrodoc-llm` if the
   crates need it) with `Scripted` (list of responses), `Recording` (received prompts) and `Counting` (call
   counter); migrate the trivial fakes, keep the specific fakes (`PeakProvider`, `SlowProvider`) in place.
7. **Large files**: turn `glossary`, `confidence`, `features`, `entry_points`, `roles` into
   `dir/mod.rs + tests.rs` like `repo_map/`. Pure move, one commit per file, `git mv` to keep the history
   readable. Do not touch `markdown.rs` before deciding whether the tests stay alongside.
8. **Constants**: make the literals uniform (`6_000`, all with separators) and add a "why this value" comment
   to the constants that lack one. No centralization in a single file (each bound belongs to its pass).

Out of scope: any functional change, any change of artifact format or prompt schema (the fingerprints would
invalidate the caches), and ADR 0006 which remains valid. If deliverable 3 or 4 changes an architecture
decision, add it to `docs/adr/`.

# Resources

* `crates/retrodoc-pipeline/src/glossary.rs`, `crates/retrodoc-pipeline/src/entry_points.rs` (point 1)
* `crates/retrodoc-cli/src/commands/` (points 2 and 5), `generate.rs` in particular
* `crates/retrodoc-pipeline/src/artifact.rs` and the `*_RELATIVE_PATH` constants (point 3)
* `crates/retrodoc-llm/src/lib.rs` (point 4)
* `crates/retrodoc-pipeline/src/repo_map/`: the split model to reproduce (point 7)
* ADR `0001` (one-way crates), `0005` (fingerprints, not to be invalidated), `0012` (request resilience)
* `just check` to validate each deliverable

# Tracking

1. [x] Shared batched read (`batched_read.rs`, `glossary` + `entry_points`)
2. [x] CLI command context (`Workspace`)
3. [x] Artifacts module (names centralized, `clear_caches` derived)
4. [x] `OpenRouterProvider::complete` split, `lib.rs` broken up
5. [x] `generate::run`: display extracted, pass guard, `GenerateOptions`
6. [x] Shared test fakes
7. [x] Large files as `dir/mod.rs` + `tests.rs`
8. [x] Constants: literals made uniform, reasons documented

## Decisions

* The deliverable 1 driver is `pub(crate)` in `crates/retrodoc-pipeline/src/batched_read.rs`.
* Test fakes (deliverable 6): `#[cfg(test)]` module internal to the pipeline crate, no `test-support` feature.
* `--force` keeps its current behavior (`roles.yaml`, `usage.json`, `scope.yaml` kept); it is documented without being changed.
