---
priority: medium
depends_on: []
why: 'the last open issue of the milestone `Business rules quality`, which can''t close without it; robustness, not seen on a run (high review of `rules_judge_prompt_feature_on_one_line`, 2026-10-31)'
---
# Keep every name of a prompt or a list on one line, once

# Goal
The pipeline now shows a feature in its prompts through one helper (`text::feature_line`) and has one `one_line`
(`text.rs`), so a name or description over several lines can't forge a numbered line. Other names still reach a
prompt or a Markdown list as they are, and the render and the MCP server each keep their own copy of `one_line`.
Close the remaining gaps, and decide whether the names are better put on one line where the passes build them.

# Findings (high review of `issues/done/rules_judge_prompt_feature_on_one_line.md`, 2026-10-31, read in the code; not seen on a run)
* The statement prompt of the rules writes the glossary entities raw (`Entities: {}`, `rules/prompt.rs`); the use
  case prompt writes the business vocabulary raw (`use_cases/prompt.rs`).
* `one_line` has private copies in `retrodoc-render` (`rules.rs`, `doc_links.rs`, a closure in
  `markdown/agent_index.rs`) and `retrodoc-mcp` (`tools/answer.rs`).
* The names and descriptions come from LLM answers and from hand-editable artifacts (`features.yaml`).

# Approach
1. Apply `one_line` to the entities and the vocabulary in the prompts, with a test each.
2. Share one `one_line` between the crates (in `retrodoc-core`, which they all depend on).
3. Or put the names and descriptions on one line where the passes read an answer or an artifact
   (`features/from_surface.rs` and the other passes), so no use has to remember it; a saved artifact would then
   need the same on load.

# Milestone
Business rules quality

# Resources
* `crates/retrodoc-pipeline/src/text.rs`, `rules/prompt.rs`, `use_cases/prompt.rs`
* `crates/retrodoc-render/src/rules.rs`, `doc_links.rs`, `markdown/agent_index.rs`; `crates/retrodoc-mcp/src/tools/answer.rs`

# Hints
* A change of the user prompt is not in the prints of the passes: no saved run is asked again, so a run saved
  before keeps its answers.

# Tracking

Plan: validated

## Understanding
Put on one line the two names still written raw in a prompt (the entities of the statement prompt, the business
vocabulary of the use case prompt), and keep one `one_line` for the workspace, in `retrodoc-core`. Out of scope:
approach 3 (one line where the passes read an answer or an artifact): it changes the artifacts, needs the same on
load, and the render keeps descriptions over several lines in the page bodies. No spec, as for the issue before it:
the prompt lines are internal and not in the prints, so no saved run is asked again.

## Plan
1. [x] Entities and vocabulary on one line: `one_line` on each entity of `rules_prompt` and each word of
   `use_cases_prompt`; verified by one test of each prompt with a name over several lines (no forged line).
2. [x] One `one_line` for the workspace: `retrodoc_core::text::one_line`, used by the pipeline (`text.rs` keeps no
   copy) and the render (the copies of `rules.rs` and `doc_links.rs` and the closure of `markdown/agent_index.rs`
   removed); the `one_line` of the MCP server, which keeps the first line cut to a snippet, renamed `snippet`;
   verified by a test of `one_line` in the core and the existing render and MCP tests.
3. [x] Fixes of the `high` review: on one line too the entry points and their outputs of the use case prompt, the
   actors and the entities of the actors prompt (and of the use case prompt, through `Actors::prompt_section`), the
   names of the agent index and of the MCP lists; the inline copies of `one_line` (brief claims, confidence rationale
   of the render, i18n values of the ingest) replaced by it; `snippet` skips leading blank lines; verified by one
   test each of the actors prompt, the use case prompt (extended), the agent index and the MCP lists.

## Decisions
- Mode: autonomous
- No approach 3 (one line at the source): the artifacts and the page bodies keep their text.
- The `one_line` of the MCP server keeps its behaviour (first line, cut to a snippet), renamed `snippet`.
- Base commit: 752b875696a5453601561283936948575f2c437b

## Summary of deliverable 1
done: `text::names_line` puts each name of a list on one line; the entities of the statement prompt and the business
vocabulary of the use case prompt go through it.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clippy pedantic clean |
| specs | n/a (no spec of the prompt lines) |
| docs | n/a (`docs/crates/pipeline.md` does not describe the prompt lines) |
| code review | `low` |

red: none traced (no spec); before the code the rules test stopped at `Entities: Order, Forged.` and the use case test
at `Business vocabulary (main entities): Invoice, Payment`, the name cut over two lines.

Code review: no findings

Not verified: none. Decisions needed: none.

## Summary of deliverable 2
done: `retrodoc_core::text::one_line` is the only one (the pipeline re-exports it, the two render copies and the
closure of the agent index removed); the MCP helper that keeps the first line cut to a snippet is named `snippet`.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clippy pedantic clean |
| specs | n/a (no behaviour changed) |
| docs | `docs/crates/core-ingest.md` names `text.rs` |
| code review | `low` |

red: none (a move: the new core test passed on its first run, the render and MCP tests cover the uses).

Code review: no findings

Not verified: none. Decisions needed: none.

## Summary of deliverable 3
done: the fixes of the `high` review of the issue (deliverable 3 of the plan).

| check | result |
|---|---|
| tests | `just check` green |
| lint | clippy pedantic clean |
| specs | n/a (no spec of the prompt lines nor of the list items) |
| docs | `docs/crates/core-ingest.md`: every other crate shares `text.rs` |
| code review | `high` on the issue, then `low` on this deliverable |

red: none traced (no spec); before the code the use case prompt showed the forged headers `=== [F4]`, `[F5]`,
`[F6]`, the actors prompt a forged `- Admin` line, the agent index a forged `- [Forged](x.md)` item and the MCP list
of domains a forged item, with a description lost behind a leading blank line.

High review: 9 findings. Fixed: the entry points of the use case prompt, the actors and entities of the actors
prompt, the names of the MCP lists and of the agent index, the inline copies of `one_line`, the crate doc, the blank
first line of `snippet`. Not fixed: `snippet` keeps its first line only (decision 2 a); the altitude finding asks for
approach 3, set aside at validation (decision 1 a).

Code review: no findings

Not verified: none. Decisions needed: none.
