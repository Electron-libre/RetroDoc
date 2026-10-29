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
