# Remove the use cases warnings of the delivery_router smoke run

# Goal

Get a `delivery_router` smoke run without any `WARN` again. Two warnings of the use cases pass showed up in
a run that was otherwise complete (render reached, rerun a no-op with no LLM call), which makes the smoke
test fail on its "no warning" criterion. They look like two separate causes, and neither comes from the
glossary or roles work of `issues/done/roles_no_model_files.md`.

# Findings (smoke run of 2026-10-07 on `delivery_router`, local Ollama, `qwen3.6:35b-a3b`)

* `human actor outside the known actors`: the actors pass identified one actor, and a use case named a
  second human actor (a rider) that is not in that list. The warning is emitted where the raw actors of a
  use case answer are turned into `Actor` values (`use_cases/grounding.rs`). On a repository where riders
  are a business actor, the actors pass probably missed one; or the use case invented it. Not checked which.
* `unparseable LLM response ... duplicate field 'steps'`: the use case answer for one feature (a rider
  assignment and routing one) had two `steps` fields in the same JSON object (answer of about 8,700
  characters); the retry then succeeded. `response.rs` parses with `serde_json`, which rejects a duplicate
  field.
* Not reproduced a second time: these may be random answers of the model.

# Approach

First measure: run the smoke test several times and count how often each warning appears.

1. Actor outside the known actors: decide whether the warning is right (a real actor the actors pass
   missed, so the fix is upstream, in the actors pass) or noise (the use case names a role that is not an
   actor of the system); then either improve the actors pass or reword or lower the warning.
2. Duplicate `steps`: make the lenient parsing (`complete_json`) accept a duplicated field, keeping the
   last or the longest one, instead of spending a retry.

# Resources

* `crates/retrodoc-pipeline/src/use_cases/grounding.rs` (actor warning), `response.rs` (lenient JSON parsing)
* `crates/retrodoc-pipeline/src/actors.rs`
* `issues/done/lenient_use_case_parsing.md`, `issues/done/smoke_delivery_router_warnings.md`
* `.claude/skills/smoke-test/`

# Hints

* The model answers differently each time: judge on several runs, not one.
* A warning that disappears by luck on one run is not a fix.
* The logs and the clone of a smoke run live in `/tmp`, never commit them.

# Tracking

1. [x] Lenient parsing accepts a duplicated field (keeps the last), `response.rs`
2. [ ] Measure both warnings over 3 smoke runs of `delivery_router`, then agree on the verdict for the actor warning
3. [ ] Actor outside the known actors: fix the actors pass if the actor is real, else lower the warning to `info`; end with 3 smoke runs without `WARN`

## Decisions

* Measure with 3 smoke runs.
* Duplicate field: keep the last one.
* Actor warning: if the actor is real, improve the actors pass; if it is noise, lower the warning to `info` and keep the actor as is.
