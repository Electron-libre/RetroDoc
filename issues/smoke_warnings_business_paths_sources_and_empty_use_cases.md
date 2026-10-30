---
rank: 1800
why: 'priority low; remeasure after the big changes'
---
# Reduce the remaining smoke warnings: invented business paths, dropped source rules and use cases without steps

# Goal

Priority: low. After `issues/done/use_cases_model_noise_warnings.md`, a `delivery_router` smoke run with the local
model still fails the "no warning" criterion on three kinds of warning that appeared in the runs that
followed it. Find out whether they are the model's random mistakes or have a cause in the pipeline, and
absorb or demote them like the others, without hiding a lost piece of documentation.

# Findings (smoke runs of 2026-10-09 on `delivery_router`, local Ollama, `qwen3.6:35b-a3b`, 2 runs)

* `ignoring business path "lib/delivery_router/route.rb": no such source` (`business_files`): 1 run of 2.
* `dropping source rule … it selects N file(s) but none can be read in that format` (`sources`): 2 rules in 1
  run of 2 (a `rails_schema` rule on `lib/**/*.rb` and an `i18n yaml` rule on `*`). The rules are the model's
  answer after the checker sent them back once.
* `use case dropped: no steps`: 2 occurrences in 1 run of 2, with an empty `use_case=` slug.
* None of them appeared in the 3 runs measured before the changes of the other issue; the second runs
  had no warning in all 5 runs. Two runs cannot tell random noise from a regression.
* A smoke run of 2026-10-16 on a large Rails application (local model, `domains_from = "surface"`, 4h19) had 15
  warnings of other kinds: 8 cited resources and 1 cited entity the surface does not know (domains pass, dropped),
  1 answer cut by `finish_reason=length`, 1 unparseable use cases answer retried, and 5 "entry points no
  feature claimed" (placed afterwards). Same question: random noise or a cause in the pipeline?

# Approach

First measure: count each kind in the saved smoke logs of `delivery_router` (listed in the planned measures of the
ledger, `CLAUDE.md`), then in the smoke runs other issues make anyway, rather than runs of its own. Those logs hold
the `WARN` lines, not the raw answers: one run with a debug log is still needed to look at the raw answer behind
each kind: was
the slug empty because the model answered an empty object, a use case with `steps: []`, or an unexpected shape?

1. Business paths: a near-miss path (closest existing source file) is resolved before being dropped, or the
   message is an `info` when the map keeps enough files.
2. Source rules: the rules the checker drops after one retry are the sniffed ones' competitors; decide if
   a dropped rule is worth a `WARN` when the sniffed map already covers the repository.
3. Use case without steps: ask once more (or say in the prompt that a use case needs at least one step),
   and keep the warning only when the answer stays empty.

# Resources

* `crates/retrodoc-pipeline/src/business_files/` (the path check)
* `crates/retrodoc-pipeline/src/sources/` (`check_rule`, `split_by_check`)
* `crates/retrodoc-pipeline/src/use_cases/grounding.rs` (`use case dropped: no steps`)
* `issues/done/use_cases_model_noise_warnings.md` once closed, for the measures and `testing::capture_logs`
* `.claude/skills/smoke-test/`

# Hints

* The model answers differently each time: judge on several runs, not one.
* Don't hide a real problem by lowering a warning: a skipped unit is lost documentation.
* `testing::capture_logs` checks the level of a log in a test (single-threaded `#[tokio::test]` only).
