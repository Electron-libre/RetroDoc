---
id: TEF
status: accepted
adrs: [0020]
---

# Technical features set aside

## Intent

On an application, the features pass groups every entry point of a domain into features, and the jobs, commands
and helpers that serve only development, tests, deployment or the internal plumbing come out as features a
reader of the functional docs does not recognise (test data, caches, short ids, health checks). The features
answer marks them technical; they are set aside out of the functional docs and the reader of the debt report
can still see what was set aside.

## Requirements

- **TEF-1** (deterministic) A feature the features answer marks `technical` must not be saved in
  `.retrodoc/cache/features.yaml`.
- **TEF-2** (deterministic) A technical feature must be saved in `.retrodoc/cache/features-set-aside.yaml`, with
  its domain, its name and its entry points.
- **TEF-3** (deterministic) An entry point set aside must not be given to the placement call of the entry points
  no feature claimed.
- **TEF-4** (deterministic) An entry point set aside must not be counted among the entry points no feature holds,
  in the coverage nor in a warning.
- **TEF-5** (deterministic) The debt report must list the technical features set aside, per domain, with the
  number of entry points of each.
- **TEF-6** (deterministic) A rerun with the same entry points, entities, brief and model must set aside the same
  features with no LLM call.
- **TEF-7** (deterministic) A feature marked both `technical` and `back_office` must be set aside as technical.

## Scenarios

- (TEF-1, TEF-2) Given a domain whose answer holds `Order checkout` and `Test data generation` marked technical,
  when the features pass runs, then `features.yaml` holds `Order checkout` only and `features-set-aside.yaml` holds
  `Test data generation` with its domain and its entry points.
- (TEF-3, TEF-4) Given the same run, when the placement step looks for unclaimed entry points, then the entry
  points of `Test data generation` are not among them and no warning names them.
- (TEF-6) Given a second run on unchanged inputs, then no LLM call is made and the same features are set aside.

## Non-goals

- Whether the model marks the right features technical is not required here: the benchmark measures it
  (`issues/done/feature_precision_on_applications.md`).
- No key of `retrodoc.toml` to put a technical feature back: of the 83 features set aside by the 6 hidden runs of
  the benchmark at f95ada6, none was a reference feature the docs lose (`issues/technical_feature_put_back.md`).
- The back-office features stay in the docs, listed apart in their domain.
