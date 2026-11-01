---
priority: medium
depends_on: []
why: 'a prompt change must redo the pass, not leave a stale cache; keeps the prompt measures honest; best after `one_line_at_the_source`, which may change the prompts (high review of `reading_hash_ignores_the_prompt`, 2026-10-29)'
---
# Make the prompt part of the prints cover everything a pass sends

# Goal
ADR 0037 puts the system prompts of the LLM passes in their prints, so a prompt change redoes the pass once.
What else a request carries is still left out: the JSON schema of the answer type and the instructions written
in the user prompts. A change to those leaves the saved answers of the old request in place until `--force`,
the stale cache ADR 0037 meant to remove. Make the prompt part cover what is sent, and keep it from being a
list kept by hand.

# Findings (high review of `issues/done/reading_hash_ignores_the_prompt.md`, 2026-10-29, read in the code; not measured)
* The schema `complete_json` sends (`response_schema::<R>()`, `response_format`) is in no print: a new answer
  field (as `back_office` on the features) keeps the saved answers without it.
* Instructions in user prompts built by `format!` are in no print: the features slice prompt ("give that
  feature again with the same slug and name", `features/from_surface.rs` near line 467), the actors filter
  ("keep those that stand for someone who acts", `actors.rs` near line 297); for the reading passes, the
  `part i/n` chunk marker and the framing of `ProductBrief::prompt_head`.
* The lists are kept by hand: `batched_read::reading_instructions` names nine constants of three modules, and
  the rules pass keeps its own hash of its three prompts (`rules/mod.rs` near line 511) rather than
  `fingerprints::prompt_part`.
* The rules pass hashes its judgement prompt with its statement prompt in one print per feature: a change of
  the judgement alone (`issues/done/rules_plumbing_and_screen_noise.md`, 2026-10-31) states the rules of every
  feature again, about 280k prompt tokens on linkding, where judging the saved `rules-stated.yaml` again would
  do (the `rules_judge_replay` example does it: about 230 short calls for seven saved runs). The judgement
  answer is not saved apart from the stated rules today (`rules-stated.yaml` holds the rules it kept).
* `prompt_part` hashes the same constants again for every unit (twice per reconciliation candidate); cheap
  (a few KB per unit) but avoidable.

# Approach
1. Compute the part of a pass from what it sends: its system prompt, the schema JSON of its answer type, and
   the fixed text of its user prompt moved to a template constant; once per pass (`LazyLock`).
2. Move the rules pass to `prompt_part` (costs one re-ask of the rules of every saved run).
3. Give the judgement of the rules its own print, so that a change of its prompt judges the stated rules
   again with no statement call (the rules before the judgement, or the verdicts, would then have to be
   saved).
4. Optionally, a test per pass that its part changes when its schema or user prompt template does.

# Specs
* `specs/rerun.md` RER-10 (widen "system prompt" to the request).

# Resources
* ADR 0037, ADR 0005; `crates/retrodoc-pipeline/src/fingerprints.rs`, `response.rs`, `batched_read.rs`,
  `features/from_surface.rs`, `actors.rs`, `rules/mod.rs`; `docs/crates/pipeline.md`.

# Hints
* Each widening re-asks the passes of every saved run once: weigh it against the measure budget
  (the measures ledger) and land the parts together.
* The golden prints of features and use cases (`the_print_of_a_*_stays_the_same`) change with it.
