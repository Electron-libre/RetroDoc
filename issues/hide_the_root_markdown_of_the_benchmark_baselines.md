---
priority: medium
why: 'every hidden figure of linkding and Lobsters so far was measured with their README read; the comparisons between settings stay valid, the absolute figures do not'
---
# Hide the root Markdown files of the linkding and Lobsters baselines

# Goal
The `hidden` series is meant to measure the docs RetroDoc writes from code alone, but it only empties
`existing_docs_paths`: the brief still reads every Markdown file of the root (`signals::root_markdown`). The linkding
and Lobsters `hidden` baselines therefore read their README and more. Expected: a `hidden.txt` for both, new
`hidden` baselines, and the size of the gap with the old ones written as a finding.

# Findings
* 2026-11-19 (`docs/findings.md` §12): the cached sources hold, at the root, `README.md`, `CHANGELOG.md`,
  `SECURITY.md` (linkding) and `README.md`, `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `SECURITY.md`,
  `sister_sites.md` (Lobsters); all feed the brief of a `hidden` run.
* `benchmark/vingilot/hidden.txt` and `benchmark.rs hide-intent` already do it for Vingilot; no list, no change.
* Lobsters' `about` page (`app/views/about/about.html.erb`) is a source of its reference's evidence: a view, not a
  doc; it stays.

# Approach
1. List the intent of each repository at its pinned commit: the root Markdown files, the docs folders the walker
   sees; decide what an inherited project would still have (a README?) and write it in each `hidden.txt`.
2. Estimate, then with the user's go, one `hidden` run per repository; compare with the last baselines of the
   ledger (domains, features, narratives), audit the new judge's pairs.
3. Write the finding; say which earlier conclusions change.

# Milestone
Read by someone taking an application over

# Resources
* `docs/findings.md` §12, skill `quality-benchmark` (`hidden.txt`), the measures ledger.
* `crates/retrodoc-ingest/src/signals/mod.rs` (`root_markdown`).
