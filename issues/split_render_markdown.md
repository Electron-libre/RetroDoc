# Split the Markdown render into its tests and one file per page

# Goal
`crates/retrodoc-render/src/markdown.rs` holds every page of the generated docs and their tests in one file,
long to read and to review. Split it along the seams it already has, with no behaviour change: `just check`
and a second `generate` writing nothing stay green.

# Findings (exploration of 2026-10-29, at 2a9c27a)
* Cut from `issues/done/split_largest_files.md` (one issue per crate, to stay under the size limit).
* `crates/retrodoc-render/src/markdown.rs`, 1,122 lines: entry points `render`, `render_with_rules`,
  `render_with_docs` at 42-159; paths and links at 160-248; one function per page (`agent_index` 249,
  `domain_page` 326, `feature_page` 377, `use_case_page` 442, `step_item` 529); tests at 565-1122 (about 556
  lines: fixtures, pages and layout, agent index, rules and doc links).

# Approach
1. Tests apart: `markdown.rs` becomes `markdown/mod.rs` (a `git mv`) plus `markdown/tests/` grouped by topic
   (about 1,110 changed lines, the tests being dedented: two deliverables, the pages and layout then the rules
   and doc links).
2. One page per file: `markdown/<page>.rs` for the agent index, the domain, the feature and the use case pages
   (about 660 changed lines).
3. One commit per deliverable.

# Resources
* `issues/done/split_largest_files.md` (the issue it was cut from, and its plan)
* `issues/done/refactor_code_hygiene.md` (how the earlier splits were made)
* `docs/crates/render-mcp-cli.md` (module list to update)

# Hints
* after split_mcp_tools
* Keep the public API of the crate (re-export from `mod.rs`); `feature_path`, `use_case_path` and `link` are
  `pub(crate)` and used by `rules.rs` and `doc_links.rs`.
* The line numbers above move with every commit; read the file again at planning.

# Tracking

Plan: validated

## Understanding
Split `crates/retrodoc-render/src/markdown.rs` into `markdown/mod.rs`, its tests by topic under `markdown/tests/`
and one file per page; pure moves, public API and `pub(crate)` helpers unchanged. Out of scope: any change of the
rendered bytes, `rules.rs`, `doc_links.rs`, `plan.rs`.

## Plan
1. [x] Pages and layout tests apart: `git mv markdown.rs markdown/mod.rs`; fixtures (`meta`, `sample`, `page`, `index_of`) in `markdown/tests/mod.rs` (`pub(super)`), the pages and layout tests in `tests/pages.rs`, the agent index, rerun and determinism tests in `tests/agent_index.rs`; the rules and doc links tests stay inline in `mod.rs` for now, renamed `mod rules_tests` (body untouched) and importing the fixtures; verified by `just check` and the moved code identical modulo whitespace (~600 lines).
2. [x] Rules and doc links tests apart: `rules_tests` → `markdown/tests/rules.rs` (dedented); verified by `just check` (~530 lines).
3. [x] One page per file: `agent_index`, `domain_page`, `feature_page`, `use_case_page` (+ `step_item`) in `markdown/agent_index.rs`, `domain.rs`, `feature.rs`, `use_case.rs`; paths, links, confidence lines and `trim_end` stay in `mod.rs` (`pub(super)` where needed); `docs/crates/render-mcp-cli.md` and `docs/ARCHITECTURE.md` point to `markdown/`; verified by `just check` (~660 lines).
4. [x] Fixes of the `high` review: the render module list of `docs/crates/render-mcp-cli.md` without nested parentheses; verified by rereading the doc.

## Decisions
- Mode: autonomous
- Base commit: 9856e71

## Deliverable 1: pages and layout tests apart
- done: `markdown.rs` → `markdown/mod.rs` (git mv); fixtures in `markdown/tests/mod.rs` (`meta`, `sample`, `page` `pub(super)` for the inline `rules_tests`, `index_of` private), pages and layout tests (with the hostile slugs one) in `tests/pages.rs`, agent index, rerun and determinism tests in `tests/agent_index.rs`; imports named; `docs/ARCHITECTURE.md` points to `markdown/`.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved lines identical (sorted line comparison) | yes, apart from imports, `mod` lines and `pub(super)` |
  | test count of the crate | 22, unchanged (17 in the render module) |
  | docs (`docs/ARCHITECTURE.md`) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: second `generate` (pure move)
- decisions needed: none

## Deliverable 2: rules and doc links tests apart
- done: the inline `rules_tests` of `markdown/mod.rs` → `markdown/tests/rules.rs` (dedented, imports named); the fixtures of `tests/mod.rs` back to private, only its children use them now.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved lines identical (sorted line comparison against 9856e71) | yes, apart from imports, `mod` lines and one assert rustfmt reflowed |
  | test count of the crate | 22, unchanged |
  | docs | nothing to change (module list unchanged at this level) |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: second `generate` (pure move)
- decisions needed: none

## Deliverable 3: one page per file
- done: `agent_index`, `domain_page`, `feature_page`, `use_case_page` (with `step_item`) moved to `markdown/agent_index.rs`, `domain.rs`, `feature.rs`, `use_case.rs` (`pub(super)`, imports named, a `//!` line each); paths, links, confidence lines, `trim_end` and the technical heading stay in `mod.rs`; `tests/pages.rs` imports `step_item` from `use_case`; `docs/crates/render-mcp-cli.md` lists the render modules, `docs/ARCHITECTURE.md` says one file per page.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | moved lines identical (sorted line comparison) | yes, apart from imports, `mod` lines, `//!` lines and `pub(super)` |
  | test count of the crate | 22, unchanged |
  | docs (`docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md`) | updated |
- red: none (pure move, no new test)
- Code review: no findings
- not verified: second `generate` (pure move)
- decisions needed: none

## High review (9856e71..afa7110)
- 10 findings: the split itself is a pure move (bodies and the 17 tests identical).
  - fixed (deliverable 4): nested parentheses in the module list of `docs/crates/render-mcp-cli.md`.
  - pre-existing, outside a no-behaviour-change issue: multi-line descriptions break the domain and feature page lists; `segment` can map two slugs to one file; inverted line range printed; duplicated item format and confidence flag across pages; rules filtered twice and deep-cloned; `ActorKind::Human` written in full. Left to the user (issue or not).
  - the issue is still open: closed on the user's agreement.

## Deliverable 4: fixes of the high review
- done: the render module list is a sentence of its own after the crate summary.
- | check | result |
  |---|---|
  | tests, lint, specs (`just check`) | pass |
  | docs | reread |
- red: none (docs only)
- Code review: no findings
- not verified: none
- decisions needed: an issue for the pre-existing findings
