---
priority: low
depends_on: []
why: 'remeasure after the big changes'
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

# Tracking

Plan: validated

## Understanding
Sort each warning kind into model noise (nothing lost: `info`) or a lost piece of documentation (`warn`, or a fix
of the cause), from the saved logs only. Out of scope: the `finish_reason=length` cut and its retry (a real limit
of the server, already handled). No new LLM run.

## Measure on the saved logs (2026-11-02, no LLM)

Logs of `~/retrodoc-runs` (smoke and benchmark runs, all repositories, local and DeepSeek). Since 2026-10-16:
about 23 runs passed through the sources pass, 44 through the business files and the use cases passes.

| Kind | Up to 2026-10-15 | Since 2026-10-16 | What it is |
|---|---|---|---|
| `ignoring business path …: no such source` | 5 in 4 runs | 8 in 3 runs | 6 are real linkding templates (not source files, the prompt leaves them out); 2 are `app/models/filter.rb` (lobsters, no such file, `tag_filter.rb` exists). Earlier: `route.rb` (`router.rb` exists), `querysets.py` (`queries.py`), `tags.py` (none) |
| `dropping source rule …` | 7 in 6 runs | 0 | not reproduced since the sources pass reads the manifests and asks schema-constrained JSON |
| `use case dropped: no steps` | 3 in 2 runs (local) | 0 | always an empty slug; raw answers not logged. `lenient_steps` drops every step that is not an object: steps written as plain strings give an empty use case |
| `domain cites a resource/entity the surface does not know` | n/a | 9 in 1 run (Rails) | some are real code names the surface does not list (workflow modules), others are inventions; nothing is lost, the resources no domain claims go to `uncategorized` |
| `entry points no feature claimed` | n/a | 6 in 1 run (Rails) | all placed afterwards: the run ends with `0 entry point(s) without a feature` |

## Plan
1. [x] Business paths: an existing file that is no source (a template) is ignored at `info` ("not a source
   file"); an invented path is `info` when the map keeps at least one entry (the empty map already warns); verified by `capture_logs` tests in `business_files/tests.rs`.
2. [x] Use cases without steps: steps written as plain strings are read as the step's text; a use case left
   with no step stays a `warn`, an entry with no slug, no name and no step (an empty placeholder) is `info`;
   verified by tests in `use_cases/tests.rs`.
3. [x] Surface passes: "entry points no feature claimed" is `info` at the slice, and a `warn` per domain only
   for the entry points still unplaced after the placement; a name the surface does not know, cited by a domain, is `info`;
   verified by `capture_logs` tests in `features` and `domains`.
4. [x] Fixes of the `high` review of the issue; verified by tests in `use_cases` and `features`.

## Decisions
- Mode: autonomous
- Base commit: aeceb65
- Answers of 2026-11-02 (all recommendations): an invented business path is `info` while the map keeps an entry,
  no near-miss resolution (`filter.rb` would become `tag_filter.rb`); an unknown name cited by a domain is
  `info`; source rules unchanged (`warn` kept, 0 in the 23 runs since 2026-10-16); no debug run, steps written
  as plain strings are read defensively.

## Deliverable 1: business paths

done: an ignored business path is logged at `info`, "not a source file" for a file that exists (a template),
"no such source" for an invented one; the empty map keeps its `warn`.

| check | result |
|---|---|
| tests | `just check` green |
| docs | `docs/crates/pipeline.md`, module doc |
| code review | low, see below |

red: none traced (no spec): `an_ignored_path_is_info_while_the_map_keeps_an_entry` failed on the two `WARN … no such source` lines

Code review: no findings

not verified: no smoke run (decided: no new LLM run).

## Deliverable 2: use cases without steps

done: a step written as its text alone is read; a step with no actor is done by the use case's
`primary_actor` (the object steps too, before dropped with "no actor"); an empty placeholder is dropped at
`info`, a named use case with no step stays a `warn`.

| check | result |
|---|---|
| tests | `just check` green |
| docs | `docs/crates/pipeline.md`, doc of `RawStep` |
| code review | low, see below |

red: none traced (no spec): the plain-text steps test panicked on `use_cases[0]` (the use case dropped), the
placeholder test on its `WARN … no steps use_case=` line

Code review: no findings

not verified: no smoke run (decided: no new LLM run); the cause of the empty slugs of 2026-10-14 stays unknown (raw answers not logged).

## Deliverable 3: surface passes

done: "entry points no feature claimed" (one domain call) is `info`, the `warn` stays on what the placement
leaves out ("entry points no feature holds"); a resource or an entity the surface does not know, cited by a
domain, is dropped at `info`.

| check | result |
|---|---|
| tests | `just check` green |
| docs | `docs/crates/pipeline.md`, doc of `build_domains_from_surface` |
| code review | low, see below |

red: none traced (no spec): the domains test failed on the two `WARN … does not know` lines, the placement
test on `WARN … entry points no feature claimed`

Code review: 1 findings

- the comment above the `info` read as if this line warned: reworded (the warning is the placement's).

not verified: no smoke run (decided: no new LLM run).

## Deliverable 4: fixes of the high review (aeceb65..HEAD)

done: placeholders are taken out of the answer before it is judged empty (an answer of placeholders alone is
asked again and warned of); a use case is dropped with a `warn` when no step survives the grounding; only a
step written as text takes the primary actor (a malformed actor stays no actor); a domain with no feature, or a
run with no feature, warns of its entry points left out; the issue no longer quotes names of the private
repository.

| check | result |
|---|---|
| tests | `just check` green |
| docs | `docs/crates/pipeline.md` |
| code review | high on the issue, then low on this deliverable |

red: none traced (no spec): the three new tests failed without the fixes (one call instead of two; `order`
kept with no step; no warning for the featureless domain)

High review: 10 findings
- placeholders alone lose the feature silently: fixed.
- a domain or a run with no feature warns of nothing: fixed.
- a use case whose steps all fall is kept empty: fixed.
- the filled actor is a name with no kind, so an unlisted name is a software component: not fixed, the
  rule of a bare name outside the actors list (`a_step_is_one_text_and_its_actor_a_name`).
- `{}` fails the whole answer (no default for `slug`, `name`): not fixed, unchanged behaviour (the answer is
  retried), no measured case.
- a malformed object actor replaced by the primary actor: fixed (text-only steps only).
- a file path with a trailing `/` kept as a folder: outside this issue, `issues/done/business_path_with_trailing_slash.md`.
- private names quoted in the issue: fixed in this file (still in the history of bfb105a..19fefd1, not pushed).
- an invented path is `info` whatever the share invented: not fixed, decision of 2026-11-02 (question 1).
- the primary actor rebuilt per step: fixed (built once).

Code review: 1 findings
- an object step with no `actor` no longer takes the primary actor: intended (finding 6 of the high review),
  it is dropped with "step dropped: no actor" as before deliverable 2, and its use case, left with no step, is
  dropped with a `warn` instead of being kept empty.

not verified: no smoke run (decided: no new LLM run).

Done 2026-11-02: every kind sorted and its level set; nothing left but the unknown cause of the empty slugs of 2026-10-14 (raw answers not logged).
