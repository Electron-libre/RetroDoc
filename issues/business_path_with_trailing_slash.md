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

* Found by the `high` review of `issues/done/smoke_warnings_business_paths_sources_and_empty_use_cases.md`
  (2026-11-02), by reading the code; no saved log shows the case.

# Approach

1. A test: an answer naming `lib/shop/order.rb/` gives the entry `lib/shop/order.rb`.
2. In `valid_entries`, try the bare path as a source file before treating it as a folder.

# Resources

* `crates/retrodoc-pipeline/src/business_files/mod.rs` (`valid_entries`)
* `crates/retrodoc-pipeline/src/business_files/tests.rs`

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
A business path written like a folder (`lib/shop/order.rb/`) whose bare form is a source file resolves to that file
instead of a folder entry. Out of scope: a spec of the business-files pass (none exists; this is a bug fix).

## Plan
1. [x] Resolve a trailing-slash file path: `valid_entries` tries the bare path as a source file before a folder;
   verified by a test in `business_files/tests.rs` (`lib/shop/order.rb/` gives `lib/shop/order.rb`). (autonomous)

## Decisions
- Mode: autonomous
- Base commit: a830704ce8a7024c351127870bc4df49419875f0
- No spec: a bug fix of a pass that has none (answer 1 a).

## Deliverable 1
- done: `valid_entries` tries the bare path as a source file first, so `lib/shop/order.rb/` resolves to the file.
- red: none traced (no spec); `left: ["lib/shop/order.rb/"] right: ["lib/shop/order.rb"]`.
- Checks: `just check` ok; size 14 lines.
- Code review: no findings
- not verified: none.
- decisions needed: none.
