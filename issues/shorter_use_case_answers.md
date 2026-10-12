# Shorten what the use cases pass writes

# Goal
The use cases pass is where the time of a run goes, and its cost is in the answer: output tokens are the
slow ones on a local model and the dear ones on a hosted one. `issues/done/use_cases_cost_per_call.md` made
the calls parallel (lever A) and left the shape of the answer (lever B) "to be reopened if the time stays
too long". Find what in the answer can be shorter, or computed without the LLM, without losing grounding or
the business reading.

# Findings (project review of 2026-10-12)
* linkding, local `qwen3.6:35b-a3b`: use cases 72.6 of 118.4 minutes (80 calls); `delivery_router`, local,
  `surface`: 8 calls take 7.5 of 12 minutes for 22k tokens in and 15k out
  (`issues/done/use_cases_cost_per_call.md`).
* linkding, `deepseek-chat`, 2026-10-12: use cases 437k tokens in and 133k out, of 242k out for the whole
  run (`issues/done/business_rules.md`).
* One call per feature writes every use case with its description, primary actor, narrative, and per step
  a description, an actor (name and kind), an action and a file with its line range
  (`use_cases/prompt.rs`). The diagram is already built without the LLM.
* Concurrency does nothing on a local server without parallel slots.
* Not measured: where the output tokens go (step 1 of the closed issue's approach was not done).

# Approach
1. Measure on saved `use-cases.yaml` files, without a call: share of the characters in narratives, step
   descriptions, actions, actor objects and citations; steps per use case; use cases per feature.
2. Candidates, one at a time against the benchmark (use cases found, business-language rate, confidence):
   (a) files cited by the number they have in the prompt instead of their path;
   (b) one text per step instead of `description` and `action`, the actor as a name (the kind derived from
   the actors list);
   (c) fewer system steps: a cap, or only the steps that change what the reader observes;
   (d) technical steps written on demand (a second, optional call), the narrative and the citations first.
3. Keep what cuts output tokens without moving the quality beyond the spread.

# Resources
* `crates/retrodoc-pipeline/src/use_cases/` (`prompt.rs`, `grounding.rs`), `diagrams.rs`, `confidence/checks.rs`
* `issues/done/use_cases_cost_per_call.md`
* `crates/retrodoc-render/src/markdown.rs`, `crates/retrodoc-mcp/src/tools.rs` (they read the steps)

# Hints
* The mechanical confidence checks read the identifiers a step names and the lines it cites: a shorter step
  must keep them.
* A new answer shape changes the fingerprints (`generate --force` once) and the saved artifact: old
  `use-cases.yaml` files must still load.
* Measure with the per-pass replay of `benchmark_passes_on_fixed_inputs` if it exists by then.
