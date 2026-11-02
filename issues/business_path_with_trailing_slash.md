---
priority: low
why: 'a rare model answer; the cost is a file listed as a folder, not lost documentation'
---
# Resolve a business file path written with a trailing slash to the file

# Goal

`business_files::valid_entries` keeps a path ending with `/` as a directory entry when a source file
starts with it. `Path::starts_with` is true for the path itself, so `lib/shop/order.rb/` (a file written
like a folder) is saved as the folder `lib/shop/order.rb/`. Expected: it resolves to the file
`lib/shop/order.rb`, or is dropped like any other path that is no source.

# Findings

* Found by the `high` review of `issues/smoke_warnings_business_paths_sources_and_empty_use_cases.md`
  (2026-11-02), by reading the code; no saved log shows the case.

# Approach

1. A test: an answer naming `lib/shop/order.rb/` gives the entry `lib/shop/order.rb`.
2. In `valid_entries`, try the bare path as a source file before treating it as a folder.

# Resources

* `crates/retrodoc-pipeline/src/business_files/mod.rs` (`valid_entries`)
* `crates/retrodoc-pipeline/src/business_files/tests.rs`
