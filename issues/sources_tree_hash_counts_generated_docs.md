# Stop the signal sources from being inferred again because of the generated docs

# Goal

`signal-sources.yaml` is inferred again when the shape of the file tree changes. That shape counts every file
the walker returns, including the docs `generate` writes, so a tree that did not change in the code can still
look new after a first run. Make the shape depend on what the sources are read from, as the business-files pass
does since `issues/done/locate_business_files.md`.

# Findings (smoke run of 2026-10-08 on `delivery_router`, local Ollama)

* The same defect made `business-files.yaml` out of date at the second `generate`: the first run wrote
  `docs/functional/…` and `docs/_retrodoc/…`, the shape hash changed, and the second run called the LLM again.
  It was fixed there by counting source files only (`source_shape_hash` in
  `crates/retrodoc-pipeline/src/business_files/mod.rs`).
* `tree_shape_hash` in `crates/retrodoc-pipeline/src/sources/mod.rs` still counts all files. It did not show
  in the smoke test, because `generate` reads the saved sources when a brief exists (`saved_or_sniffed`), so
  it is only reached by `retrodoc brief` after a `generate`, or with a brief that was removed. Not
  reproduced on a real run.

# Approach

First reproduce: `generate`, then `retrodoc brief` without `--force`, and check whether it infers the sources
again (the usage recap shows a `sources` call).

Then, candidates:

1. Count only the files a source rule can select (schema, migrations, translations): non source files outside
   the formats read, and the generated docs dir, are left out of the shape.
2. Share one shape function with the business-files pass, parameterized by the kinds of file that count.
3. Leave the generated docs dir (`output.docs_dir`) out of the walk for every pass that hashes the tree.

# Resources

* `crates/retrodoc-pipeline/src/sources/mod.rs` (`tree_shape_hash`, `infer_sources`)
* `crates/retrodoc-pipeline/src/business_files/mod.rs` (`source_shape_hash`) and its test
  `files_written_by_generate_do_not_make_the_map_out_of_date`
* ADR `0019`, ADR `0025`

# Hints

* Translations and schema files are often not `FileKind::Source` (YAML, SQL), so the business-files fix does
  not apply as is: the shape must include the kinds the rules read.
