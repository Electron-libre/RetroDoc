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
