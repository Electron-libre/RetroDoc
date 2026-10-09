# Reduce the model-noise warnings of the use cases and chunking passes

# Goal

Make a `delivery_router` smoke run with the local model end without `WARN` more often. Three smoke runs of
2026-10-07 each failed the "no warning" criterion on different warnings, none of them the two handled in
`issues/use_cases_smoke_warnings.md`. They look like the model's random mistakes, so the goal is to absorb
them cheaply (lenient parsing, one more retry, a repair step) or to decide which ones are not worth a
`WARN`.

# Findings (smoke runs of 2026-10-07 on `delivery_router`, local Ollama, `qwen3.6:35b-a3b`)

* Run 1: `chunk boundary misses definitions` (the boundary regex proposed for `rb` covered 0% of the
  definitions; the LLM was asked to fix it, which worked).
* Run 2: four warnings. The use case answers of two features were unparseable on the first attempt (`expected
  value`, `EOF while parsing an object`: answers of 1,500 to 3,000 characters, probably cut or malformed;
  the retry worked). The confidence verdict of 2 use cases was unparseable twice and skipped (`EOF while
  parsing an object`, then `expected value`; answers of about 1,000 characters).
* Run 3: two `step reference dropped: file not in the feature` warnings, on paths the model invented
  (a misspelled file name, a wrong directory name).
* The second `generate` of each run had no warning.
* The warnings differ from one run to the next: random, not a single defect.

# Update (2026-10-09): what the schema settled

`issues/done/structured_llm_outputs.md` measured six runs on the same repository. With the JSON schema, the
unparseable answers went from 6 (one unit skipped) to 0, so points 1 and 2 below (unparseable JSON, a
verdict skipped twice) are settled for a server that accepts the schema; keep the lenient parsing for
those that do not. What stays is the semantic noise, which a schema cannot prevent: a use case citing an
entry point that does not exist (`use case cites an unknown entry point, dropped`, with values like
`orders`, `rider`, or a path with a line number) and a business path that does not exist. The goal now is
points 3 and 4, plus these entry-point references.

# Approach

First measure: count each kind over several runs, and check whether a truncated answer comes from the
output limit (`finish_reason=length`) or from the model.

1. Unparseable JSON: look at the raw answers (debug log) and extend the lenient parsing for what recurs
   (cut object, trailing comma, a missing value), or raise the retries for a unit whose answer is cut.
2. Confidence verdict skipped twice: decide whether a skipped sample should be a warning, and whether the
   unit deserves a third attempt.
3. Invented step paths and entry points: try to resolve a near-miss path (closest file of the feature) before dropping the
   reference; keep the warning only for what can't be resolved.
4. Chunk boundary: decide whether a first-try 0% coverage is worth a `WARN` when the fix succeeds (an `info`).

# Resources

* `crates/retrodoc-pipeline/src/response.rs` (lenient parsing, retries)
* `crates/retrodoc-pipeline/src/use_cases/grounding.rs` (step references)
* `crates/retrodoc-pipeline/src/confidence/`, `crates/retrodoc-pipeline/src/chunk_check.rs`
* `issues/use_cases_smoke_warnings.md`, `issues/done/lenient_use_case_parsing.md`
* `.claude/skills/smoke-test/`

# Hints

* The model answers differently each time: judge on several runs, not one.
* Don't hide a real problem by lowering a warning: a skipped unit is lost documentation.
* The logs and the clone of a smoke run live in `/tmp`, never commit them.
