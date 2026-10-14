# Record in every use case the entry points it covers

# Goal
What share of the behaviours of an application do the use cases reach? Today the answer cannot be measured:
a use case carries an `entry_points` field but it is empty for most of them. Fill it in, and use it to report
the entry points no use case covers.

# Findings (first real run on a large Rails application, 2026-10-13)
* 49 of 161 use cases have a non-empty `entry_points`; the 112 others have none, although their steps cite the
  code they come from.
* A feature of 76 entry points (notification classes) got 11 use cases, one call: the answer of its use cases
  was cut at the output limit twice in the first run. Which of the 76 are described, nobody can say.
* The earlier measure of the share of files a behaviour reaches (`behaviour_baseline`) works from the entry
  points and the slices, not from what the use cases say.

# Approach
1. Make the use cases answer cite the ids of the feature's entry points they cover (as the features answer does),
   check the ids mechanically and drop the unknown ones.
2. Report, per feature and overall, the entry points covered by no use case, in the coverage report and
   `behaviour-coverage.yaml`.
3. For a feature with many entry points, ask for the use cases in several calls over slices of its entry points,
   so no answer is cut (together with `issues/shorter_use_case_answers.md`).

# Resources
* `crates/retrodoc-pipeline/src/use_cases/` (prompt, grounding), `behaviour_coverage.rs`
* `issues/shorter_use_case_answers.md`, `issues/done/validate_on_large_application.md`

# Hints
* The application is confidential: no name, path or code of it in a committed file.

# Tracking
1. [x] The use cases answer cites entry point ids (`<file>#<name>`); `known_entry_points` validates them (a bare name still works when it is unambiguous), drops the unknown ones.
2. [x] Entry points covered by no use case, per feature and overall: `behaviour-coverage.yaml` (optional field, old files still load) and the debt report.
3. [x] A feature with many entry points gets its use cases in several calls over slices of its entry points (constant, 12 per call, grouped by file), results merged by slug.
4. [x] Docs (`docs/crates/pipeline.md`, `PLAN.md`) and a real `generate --force` run on a large application to measure the covered share (generic wording, no name).

## Decisions
* Ids are the full `<file>#<name>`, as in the features pass.
* Entry points already saved by name are not migrated; `generate --force` once.
* The slice size is a constant, no new `retrodoc.toml` key; adjust after a real run.
* The uncovered entry points go to the debt report as well as to `behaviour-coverage.yaml`.
* Runs are kept in `~/retrodoc-runs`, not `/tmp`.
* Deliverable 1: the prompt shows tags `E1`.. (as the features pass) and the answer cites them; ids are stored. Not measured on a real model yet (deliverable 4).
* Deliverable 2: `by_feature` in `behaviour-coverage.yaml`, "Entry points without a use case" in the report. Known limit: the entry of a deleted or renamed feature is not pruned (a partial run does not know all the features); `generate --force` wipes the artifact.
* Deliverable 3: slices of whole files (`file_ranges`, shared with the features pass), asked one after the other, no merge across slices. Known limits: a form and its submit route in two slices give two use cases; an unusable slice redoes the whole feature next run; the constant is not in the fingerprint (`--force` after changing it). Not measured on a real model yet (deliverable 4).
* Deliverable 4, real run (2026-10-14, a large Rails application, local `qwen3.6:35b-a3b`, `domains_from = "surface"`, 1h06): 52 use cases for 5 features, 48 with a non-empty `entry_points` (92%, against 49 of 161 before). They cite 68 of the 123 entry points the features hold; 55 (45%) are cited by none (per feature, from 4 of 10 to 14 of 18). The feature with 73 entry points was asked in slices: 14 calls for 5 features, no answer cut, use cases 24m31s for 157k tokens in and 26k out. The second run made no call. The smoke verdict is FAIL for 107 warnings in the first run, 94 of them the placement pass dropping a placement "in an unknown feature" (`issues/placement_into_unknown_features.md`). Not measured: why the 55 are not cited (omissions of the model, or entry points no use case needs), nor the run with the code before this issue.
