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

# Tracking

Plan: validated

## Understanding
Every LLM pass declares its request once (system prompt, answer type, fixed user prompt texts as constants);
the call and the print both use it, so a schema or user-text change redoes the pass once (RER-10). The rules
pass gets a statement print and a judgement print; a judgement change re-judges the saved statements (RER-11).
Out of scope: the hand-editable artifacts (roles, sources, business-files, brief, contexts), the benchmark replays.

## Plan
1. [x] Request and reading passes: a `Request<R>` (system, user texts, schema of `R`, part hashed once per
   process) taken by `complete_json`; glossary, business, entry points and joint reading moved to it, the brief
   framing in its print part, `reading_instructions` built from the four requests; verified by unit tests of the
   part (changes with the schema, with a user text) and the reading pass tests; makes true RER-10 (reading).
2. [x] The other passes: domains, features (slice text), placement, use cases, actors (filter text),
   reconciliation, confidence moved to their requests; golden prints updated; verified by a test per pass
   that a saved print of the old part is asked again; makes true RER-10.
3. [x] Rules: statement and kind of feature on one request part, judgement on its own print; a feature whose
   judgement print alone changed is judged again from `rules-stated.yaml` and the rules set aside with their
   whole rule; verified by a test counting the calls (one judgement, no statement, then none); makes true RER-11.

## Decisions
- Mode: autonomous
- Base commit: e604dc8754244c9965625b0fc53d32ce265cf6cf
- User prompt texts: constants declared in the request of each pass (not a sample render, not the prompt sent).
- Cost: the three deliverables land together before any measure (one re-ask of every saved run).
- Rules before the judgement: rebuilt from `rules-stated.yaml` and the rules set aside, no new artifact.

## Deliverable 1
- done: `request.rs` (`Request<R>`: system, user texts, schema; part hashed once), the four reading requests,
  `reading_part` (requests, file frame, part marker) replacing `reading_instructions`; spec RER-10/RER-11 and
  ADR 0040 drafted.
- checks: tests ok, clippy ok, specs ok (RER draft), docs `pipeline.md`.
- red: RER-10: `assertion left != right failed` (a part from the system prompt alone did not change with the
  schema).
- Code review: no findings
- The brief framing is already covered: `ProductBrief::fingerprint` hashes the rendered section, titles included.

## Deliverable 2
- done: domains, features (the later-slice instruction), placement, use cases (`ask_with_system`, the system
  addenda as texts), actors (the entity filter), reconciliation (part cached, no longer hashed per candidate),
  confidence (two requests in `confidence_model`) on their `Request`; golden prints updated; docs.
- checks: tests ok, clippy ok, specs ok, docs `pipeline.md`, `ARCHITECTURE.md`.
- red: RER-10: the golden prints `the_print_of_a_domain_stays_the_same` and `the_print_of_a_feature_stays_the_same`
  failed (`left: fc0320be… right: a1766605…`), the print now holding the schema and texts.
- Code review: no findings
- Decision: the layout of an item of input (`- name: description`, `Feature:`, `Steps:`) stays inline and is
  not in the print; only instructions and section headers are texts of a request (doc of `Request`).

## Deliverable 3
- done: rules on `STATEMENT`, `KIND`, `JUDGEMENT` requests; print of a feature `"<statement> <judgement>"`;
  a feature whose statement part matches is judged again from `rules-stated.yaml` and the rules set aside
  (sorted by number), or reused when it has none; a rule set aside without its whole rule states it again.
  Spec RER accepted, ADR 0040 accepted.
- checks: tests ok, clippy ok, specs ok, docs `pipeline.md`, ADR 0040.
- red: RER-11: `called Option::unwrap() on a None value` (the saved print had no judgement part).
- Code review: 3 findings, none fixed after checking:
  1. rules set aside for another reason judged again: only the judgement creates an `AsideRule` (`rules/mod.rs`
     `judged`), so every one was dropped by it.
  2. `Plan::Reuse` keeping the old judgement part: `per_feature::run` stores the unit's new print on reuse.
  3. old prints with no space state every feature again: intended (RER-11 scenario, no migration); prints are hex.
