# Decide whether a prompt change should re-read the files of a saved run

# Goal
The per-file hash of the reading passes (`glossary`, `entry_points`, `business_read`) covers the content of the
file, the brief and the model, not the prompt. A prompt change then re-reads nothing in a saved run nor in the
frozen inputs of the replays: the old entries stay until the file changes or the run is forced. Decide whether
the prompt belongs in that hash, and make the passes consistent with the decision.

# Findings (read in the code on 2026-10-29, at fd1ed83; not measured)
* `crates/retrodoc-pipeline/src/batched_read.rs:140`: `hash_for_model(&brief.hash_with(&content), llm)`.
* The passes disagree: the rules fingerprint holds its prompts ("a new answer shape asks the saved features again
  once"), the features fingerprint does not ("the prompt is not part of the fingerprints, so a run saved before
  this key keeps `false` until `--force`"), per `docs/crates/pipeline.md`.
* Found while measuring `issues/done/joint_reading_routing_rule.md`: a measure of a prompt change on the reading
  passes has to clear their cache first, or it compares the old answers with themselves.

# Approach
1. List, pass by pass, what each fingerprint or per-file hash covers and whether it holds the prompt.
2. Candidates (can be combined): put a version or hash of the prompt in every fingerprint, so a prompt change
   re-reads once (cost: a full re-read on the next run after each prompt change); keep it out and give the
   replays a way to ignore the cache of a pass; write the rule down in an ADR either way.

# Resources
* ADR 0005 (incremental re-run, fingerprints), ADR 0028
* `crates/retrodoc-pipeline/src/batched_read.rs`, `business_read/`, `entry_points/`, `glossary/`
* `docs/crates/pipeline.md`, `specs/rerun.md`

# Hints
* A rerun that calls no LLM is a criterion of the smoke test: a prompt hash must not change between two runs of
  the same binary.

# Tracking

Plan: validated

## Understanding
Put a hash of the fixed instructions of each LLM pass in what it compares per unit, next to the model, so a
prompt change redoes that pass once (RER-10, ADR 0037 amending 0005). Out of scope: the hand-editable artifacts
(`roles`, `sources`, `business-files`, `brief`, `contexts`), kept until `--force`; no migration of saved runs.
Inventory at 2de90d2: only `rules` holds its prompts, `reconcile` a hand-bumped `SHOWN_RULE`; `glossary`,
`entry_points`, `business_read` (per-file hash), `domains`, `features`, placement repair, `use_cases`, `actors`,
`confidence` (`confidence_model`) hold input and model only. Replays are not concerned: the replayed pass runs
with no fingerprints (`benchmark/frozen.rs`).

## Plan
1. [x] Reading passes: `fingerprints::prompt_part`, taken by `batched_read::pending_chunks` from `glossary`,
   `entry_points`, `business_read`; spec and ADR drafts committed; verified by a test that a file saved under
   other instructions is pending and is not on a second run; makes true RER-10 (~200 lines).
2. [x] Other passes: `domains`, `features`, placement repair, `use_cases`, `actors`, `confidence`, `reconcile`
   (its system prompt next to `SHOWN_RULE`), `rules` through the same helper; docs (`docs/crates/pipeline.md`,
   `fingerprints.rs` `//!`) say the prompt is compared; verified by a test per pass that a unit saved under
   other instructions is asked again; makes true RER-10, spec `accepted` (~400 lines).

## Decisions
- Mode: autonomous
- Base commit: 2de90d29c2936515d729b7fb3e7c72bee8086741
- Detection: an automatic hash of the prompt text, as `rules` does (no hand-bumped version).
- Saved runs: no migration, asked again once.
- Deliverable 2: only the system prompts of the other passes are hashed (their user prompts' fixed text is
  spread in `format!` calls); `rules` keeps its print as it was, to spare a re-ask with no gain. RER-10 reworded
  to say so; spec `accepted`, ADR 0037 `Accepted`.
- Deliverable 2 tests: one per pass whose old print a test can rebuild (confidence, reconcile); the golden
  prints of features and use cases updated, their comment now says a prompt edit changes them on purpose.

## Deliverable 1
- done: the per-file hash of `glossary`, `entry_points`, `business_read` takes `reading_instructions()` (their
  system prompts, headers and file marker, one part for all three since the joint reading saves what the other
  two reuse); `fingerprints::prompt_part`; spec RER-10 and ADR 0037 drafts.
- checks: tests ok, clippy ok, specs ok (warnings on untouched requirements), docs `pipeline.md`.
- red: RER-10: `pending_chunks` took no instructions (E0277, the test did not compile).
- not verified: no run on a saved repository.

## Deliverable 2
- done: system prompt hashed in the prints of `domains`, `features`, placement, `use_cases`
  (`SYSTEM_PROMPT_PARTS`), `actors`, `reconcile` and in `confidence_model` (`scored_by`); docs `pipeline.md`,
  `ARCHITECTURE.md` §5, ADR 0037, spec RER-10.
- checks: tests ok, clippy ok, specs ok, docs updated.
- red: RER-10: confidence `left: [1, 0, 0, 0] right: [1, 1, 0, 0]`; reconcile `left: [0, 0]` (expected `[1, 0]`).
- not verified: no per-pass test for domains, features, placement, use cases, actors (their print before the
  change can't be rebuilt from a test); covered by the golden prints of features and use cases only.
- Code review (low): no findings.
