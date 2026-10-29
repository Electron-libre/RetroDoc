# Fix the edge cases of the MCP tools' answers and of the `Docs` builders

# Goal
The `high` code review of `issues/split_mcp_tools.md` (2026-10-29, `3dfec60..b71dbf0`, pure moves) found
behaviours of the MCP tools that predate the split: ids resolved on part of their segments, malformed or empty
answer text, builders whose result depends on their call order, and lookups that scale badly. Decide which are
bugs, fix those, and leave the intended ones documented, with no change to the answers of well-formed ids.

# Findings (code review of 2026-10-29, at b71dbf0)
* `crates/retrodoc-mcp/src/tools/get_feature.rs`: `get_feature` matches only the last segment of the id. A wrong
  domain prefix (`shipping/pay-invoice`) still answers with `billing/pay-invoice`, and when two domains share a
  feature slug, only the first one can be reached. The doc comment accepts "its slug alone" on purpose, so
  the leniency may be intended; the unreachable duplicate is not.
* `crates/retrodoc-mcp/src/tools/get_use_case.rs`: `get_use_case` reads only the last two segments, so a wrong
  domain segment still resolves, and it takes the first match when two domains share a feature slug.
* `crates/retrodoc-mcp/src/tools/answer.rs`, `unknown`: with no domain generated, the answer reads
  "Known domains: ." with an empty list.
* `answer.rs`, `one_line`: takes the first line before trimming, so a description that starts with a blank line
  gives an empty snippet in `list_domains`, `get_domain`, `get_feature`.
* `answer.rs`, `entry_set_apart`: for each use case search hit, scans every use case × feature and builds a
  `format!` string per pair.
* `answer.rs`, `stale_files`: deduplicates the paths with `Vec::contains` inside a filter (quadratic).
* `crates/retrodoc-mcp/src/tools/mod.rs`, `with_source_access`: builds the allow-list from `self.rules` at call
  time, so calling it before `with_rules` silently leaves out the files cited only by rules (`Docs::load`
  calls them in the right order).
* `mod.rs`, `with_rules`: appends the rule entries to the index, so a second call leaves the first rules
  searchable while `get_*` no longer knows them.
* Not measured: whether any of these shows up on a real generated corpus (duplicate feature slugs across
  domains, descriptions starting with a blank line).

# Approach
1. Measure first: count the duplicate feature slugs across domains and the descriptions starting with a blank
   line in the saved artifacts of a kept run (no LLM call).
2. Ids: when the id carries a domain or feature segment, check it; answer "Not documented" on a mismatch, and
   list the candidates when a slug alone is ambiguous. Keep the slug-alone lookup if it is unambiguous.
3. Answer text: drop the "Known domains" clause when the list is empty; take the first non-blank line in
   `one_line`.
4. Builders: make the order irrelevant (compute the allow-list lazily or in one `build` step) and make
   `with_rules` replace rather than append, or merge the builders.
5. Lookups: index features by id and use cases by `(feature, slug)` for `entry_set_apart`; a `HashSet` in
   `stale_files`.

# Specs
* No spec covers the MCP server yet; write `specs/mcp-server.md` for the id resolution (step 2) at planning,
  from the current tests of `crates/retrodoc-mcp/src/tools/tests.rs`.

# Resources
* `issues/split_mcp_tools.md` (where the review ran) and `issues/done/mcp_server.md` (the design of the tools)
* `crates/retrodoc-mcp/src/tools/` and `docs/crates/render-mcp-cli.md`

# Hints
* Steps 3 to 5 do not change the answer of a well-formed id; step 2 does, so it is the one that needs a spec.
