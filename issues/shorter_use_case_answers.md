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

# Tracking

Plan: validated

## Understanding
Cut the output tokens of the use cases answer without moving use cases found, business language (judge) or
grounding beyond the spread of the replays. Out of scope: the input side (excerpts), batching features, a
spec (the answer shape is internal; the saved artifact keeps loading). Candidate (d) (a second call) is left
out: it adds a call and only saves when the steps are not wanted.

## Measure of step 1 (2026-10-27, no call)
Answers rebuilt in the prompt's JSON shape from saved `use-cases.yaml`
(`examples/use_cases_answer_size_measure.rs`). Share of the characters of the answer:

| run | use cases / feature | steps / use case | system steps | step description | citations (of which paths) | narrative | name, slug, description | actor object | action |
|---|---|---|---|---|---|---|---|---|---|
| linkding frozen (81 uc) | 2.5 | 4.6 | 83% | 21% | 19% (8%) | 18% | 10% | 8% | 6% |
| linkding `rules-ld-judged` (141) | 3.3 | 4.3 | 64% | 20% | 20% (10%) | 18% | 9% | 8% | 8% |
| a private Rails app, deepseek (762) | 9.0 | 3.3 | 75% | 19% | 17% (9%) | 20% | 11% | 8% | 7% |
| `delivery_router` frozen (5) | 1.0 | 5.8 | 100% | 21% | 24% (10%) | 16% | 9% | 8% | 6% |

The JSON keys are the remaining ~16%. Steps (description, action, actor, citations) are about 55% of the
answer; the narrative, the business level, under 20%.

Estimated size of the answer under each candidate shape, against the current one:

| run | (a) compact citations | (b) one text, actor name | (c) at most 5 steps | (a+b+c) |
|---|---|---|---|---|
| linkding frozen | -18% | -16% | -6% | -37% |
| linkding `rules-ld-judged` | -19% | -18% | -9% | -40% |
| a private Rails app | -16% | -16% | -3% | -34% |
| `delivery_router` frozen | -22% | -16% | -17% | -45% |

The cap alone of (c) cuts little where use cases have few steps; what the model leaves out under the
instruction "only the steps the reader observes" is not estimable without a call.

## Plan
1. [x] Measure example `use_cases_answer_size_measure`: the table above from run dirs, plus the estimated
   size of the answer under each candidate shape (a, b, c); verified by a unit test on a small artifact.
2. [x] Grounding in the use cases replay: `UseCaseFigures` gains the steps with a citation and the share the
   mechanical checks pass (`mechanical_score`), shown by `use_case_table`; verified by a test of the table.
3. [x] Baseline: one `pass_replay use-cases` on frozen linkding, `deepseek-chat`, current shape.
4. [x] (a) Compact citations: files numbered in the prompt, a citation as `"F2:10-24"` (the rules pass's form, its reading moved to `citations.rs`); a
   path still accepted; verified by grounding tests (number to path, unknown number dropped); one replay,
   kept if within the spread.
5. [x] (b) One text per step and the actor as a name (kind from the actors list, else `System` / human
   by the list); verified by grounding and render tests (old artifacts with `action` still load and render);
   one replay, kept or reverted.
6. [x] (c) Fewer system steps: only the steps that change what the reader observes, with a cap; verified by a
   prompt test; one replay, kept or reverted.
7. [ ] Docs (`docs/crates/pipeline.md`, `llm.md` if touched), the `generate --force` note, findings recorded.

## Decisions
- Mode: autonomous
- Base commit: d64d803c02c4c8643d777b251e68c4046f2a5070
- Paid replays: one run per shape on frozen linkding, `deepseek-chat` (user, 2026-10-27). With one run the
  spread is not measured: it is read from the replays of `issues/done/benchmark_passes_on_fixed_inputs.md`.
- (b): the one text goes to `description`, `action` stays empty; the diagram (truncated), the render and the
  MCP fall back to the description (user, 2026-10-27).
- (c): only the steps that change what the reader observes, at most 5 steps per use case (user, 2026-10-27).

## Summary of deliverable 1
- done: example `use_cases_answer_size_measure` (fields, steps, system steps, candidate shapes), line in
  `docs/dev/commands.md`, figures above.
- red: none (a new example; its two tests written with it).
- Code review: (b) credited with the `refs` key of (a), fixed; bytes counted as characters, fixed.
- not verified: the estimates assume a file number of two digits and steps the grounding did not drop.

## Summary of deliverable 2
- done: `UseCaseFigures` gains `steps`, `steps_cited`, `steps_passing` (`mechanical_score` with the glossary
  entities, as the confidence pass); three columns in `use_case_table`; `commands.md`, `pipeline.md`.
- Code review: no findings.
- red: `no field steps on type UseCaseFigures` (the replay test asserting 2 steps, 1 cited, 1 passing).
- not verified: on a real replay (deliverable 3).

## Replays (frozen linkding, `deepseek-chat`, one run each)

| shape | use cases | business language, judge | score = 1 | steps | cited | checks pass | calls | prompt tokens | completion tokens | seconds |
|---|---|---|---|---|---|---|---|---|---|---|
| current (2026-10-27) | 80 | 79% | 65% | 349 | 100% | 68% | 32 | 275,026 | 34,146 | 176 |
| (a) `"F2:10-24"` | 80 | 95% | 70% | 346 | 100% | 68% | 32 | 276,602 | 27,426 (-20%) | 153 |
| (a+b) one text, actor name | 81 | 81% | 67% | 351 | 100% | 66% | 32 | 276,378 | 23,679 (-31%) | 145 |
| (a+b+c) observable steps, 5 at most | 81 | 73% | 67% | 288 | 100% | 69% | 32 | 277,978 | 21,667 (-37%) | 140 |

## Summary of deliverables 3 and 4
- done: baseline replay; (a) kept: steps cite `"F2:10-24"`, the files are numbered in the prompt, the
  reading of citations shared with the rules pass (`citations.rs`); `source_refs` objects and paths still
  read. Completion tokens -20%, as estimated; use cases, steps and grounding unchanged; the judge's 95%
  against 79% is one run each, read as noise until (b) and (c) say otherwise.
- Code review: no findings.
- red: the prompt did not ask for `"code":["F1:10-24"]` (test `steps_cite_the_files_by_their_number_in_the_prompt`).
- not verified: a local model (only `deepseek-chat`); the prompt is not in the fingerprints, so saved use
  cases keep their shape until `generate --force`.

## Summary of deliverable 5
- done: (b) kept: a step is `{"actor": "<name>", "text": "...", "code": [...]}`; the text goes to
  `Step.description`, `Step.action` stays empty (`#[serde(default)]`, `Step::label` falls back on the text);
  the actor's kind comes from the actors list, else system; the diagram cuts the text at 60 characters on a
  word; render, MCP and confidence prompt drop the empty action; old steps still read. Replay: completion
  tokens -14% after (a), -31% in all; use cases, judge and grounding within one run's noise (checks pass 66%
  against 68%). `pass_replay` keeps the use cases of a run with `RETRODOC_KEEP_USE_CASES`.
- Code review: no findings (two passes, the second after the prompt with no actors list).
- red: `no method named label found for struct Step` (and the new tests of grounding, diagram and render).
- not verified: the share of human steps under (b) (the replay did not keep its use cases); checked on the
  run of (c), which reads the actors the same way. With no actors list, the prompt asks for the
  `{"name", "kind"}` object again (`ACTOR_KIND_ADDENDUM`), so a person keeps its kind.

## Summary of deliverable 6
- done: (c) kept: the prompt asks for the steps that change what the actor or the reader observes, at most 5
  per use case (no cap in the code). Replay: completion tokens -8% after (b), -37% in all, the pass 140 s
  against 176; steps 288 against 351, none over 5 (19 of 81 before); checks pass 69%; human steps kept
  (53 of 288, all `End-user`, against 65 of 375 in the frozen use cases), which also checks (b).
- Code review: no findings.
- red: the prompt did not say "at most 5 steps" (test `the_prompt_asks_for_the_steps_the_reader_observes_five_at_most`).
- not verified: the judge's business language reads 73%, under the 79-95% of the three earlier shapes;
  the narrative prompt is untouched by (c) and the judge varied by 16 points between (a) and (b) alone,
  so it is read as the judge's noise; three runs of (a+b+c) would settle it (about $0.6).
