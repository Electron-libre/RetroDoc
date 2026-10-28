# Decide whether `domains_from = "surface"` becomes the default

# Goal

Settle the default of `[pipeline] domains_from` (ADR 0020) with enough evidence. `surface` cuts the calls and
the time on a large application but its features were worse on a 9-file library; the default stays `files`
until that gap is understood and a second large repository agrees.

# Findings

* Benchmark of 2026-10-10 (`issues/done/behaviour_coverage_domains.md`, "Comparison"): on linkding with deepseek,
  `surface` makes 110 calls in 6.4 minutes against 222-283 in 9.7-14.4, with judged feature recall at par and
  better narratives; on `delivery_router` with deepseek, judged feature precision is 45% against 96% and the
  recall of the three runs ranges from 12% to 88%.
* Not measured: linkding with the local model, the `shown` series, the confidence pass on linkding, a second
  large repository.
* The path depends on the entry points found: on `delivery_router` the share of files reached swings between
  44% and 100% from run to run on the old path too.

* Reading of the three weak `delivery_router` deepseek runs (2026-10-10): the entry points pass finds 13 to 21
  entry points depending on the run, including constructors (`Customer.new`, `Order.new`) and the methods of
  internal collections, not only the public API of the library (the router's `add_order`, `clear_orders`,
  `delivery_time`, `route`). The features then follow the classes ("Customer creation", "Rider management")
  instead of the capabilities of the reference. The gap comes from the entry points pass, not from the grouping.

* The first complete work on a large application (a private Rails application of 1,676 source files,
  `issues/done/validate_on_large_application.md`, 2026-10-13) used the `surface` pipeline in a third mode,
  `domains_from = "contexts"` (ADR 0029): the entities and entry points of a bounded context at a time, with
  the readers adding to their inventories. It finished and cost about $1.3 for a third of the application;
  the `files` path was not tried on it (the repo map alone is one call per file). So the default question now has three values.
* `--max-files` does not bound the `surface` path: `generate` applies the budget after the surface passes, and the
  features are built from every entry point. With `domains_from = "contexts"` the flag is the reading budget of a run.

* Saved runs read again on 2026-10-28 (no LLM call). Lobsters (Rails, 1 run of `generate` with `files`, and the
  `surface` passes replayed 3 times on its frozen inputs, all at d8d8c92 with deepseek-chat, so with one
  scoring, before the merged-features scoring of da35abe):

  | lobsters | `files` (generate, 1 run) | `surface` (pass_replay, 3 runs) |
  |---|---|---|
  | domains | 8 | 8 (8-9) |
  | domains recall / precision, judged | 43% / 38% | 48% (29-57) / 45% (29-57) |
  | features | 98 | 77 (77-78) |
  | features recall / precision, judged | 81% / 51% | 87% (84-90) / 49% (42-56) |
  | calls of repo map + domains + features | 185 + 1 + 10 (of 351) | none + 1 + 9 |
  | prompt tokens of those passes | 180k + 5k + 22k (of 1.38M) | none + 3.7k + 26k |
  | time of those passes | 252 s + 4 s + 44 s (of 18 min) | 5 s + 27 s |

  The `surface` features are built on the first replay's domains (one draw), so the `surface` path would save the
  repo map, about half the calls and 4 of the 18 minutes; deliverable 2 checks it on a whole `surface` generate. On this second
  large application the `surface` path is at par or better, like on linkding.
* delivery_router: the `surface` replays of 2026-10-23 (f7ac334) saturate at 100% on domains and features, but
  their entry points were frozen from a local qwen run, not from deepseek's entry points that made the weak
  runs, the outputs are lost and the judge got 10 of 23 feature pairs wrong there; delivery_router is a smoke
  repository only since (`issues/done/benchmark_passes_on_fixed_inputs.md`). The library gap is therefore not
  settled by them: deliverable 2 reruns the whole pipeline with deepseek on both paths.

# Approach

1. Find why the library does worse: read the entry points, domains and features of the three weak runs
   (a library's public API as entry points, ADR 0008; features that group too many or too few entry points).
2. Fix what the reading shows, or accept that a tiny library is outside the target.
3. Run the benchmark with more runs (5) on `delivery_router`, once on linkding with the local model, with the
   `shown` series and with confidence on, and on a second public application.
4. Decide the default; update `PLAN.md`, `CLAUDE.md`, the ADR and the default config written by `init`.

# Resources

* `issues/done/behaviour_coverage_domains.md` (comparison tables), `benchmark/`, skill `quality-benchmark`
  (`--domains-from surface`), ADRs `0008` and `0020`.

# Hints

* "Previous" in the benchmark table is the last benchmark of the repository whatever its model or path:
  compare like with like by hand.
* The local linkding run takes 1 to 2 hours; paid runs need an explicit go.

# Decisions

* Parked (user, 2026-10-10): this is an optimization. The default stays `files`; `surface` stays opt-in.
* Not done: fixing the entry points of a library (no dedicated issue either), a second public application,
  paid runs (the extra benchmark of linkding with confidence and the `shown` series costs about $5-6),
  local runs of linkding.

# Tracking

Plan: validated

## Understanding
Settle the default of `domains_from` (`files`, `surface`, `contexts`) on evidence, reusing first what was measured
since the parking (lobsters `files` generate vs `surface` replays, delivery_router replays at f7ac334), then a few
cheap paid runs. Out of scope: removing the losing path (`single_pipeline_path`), local linkding runs.

## Plan
1. [x] Evidence from saved runs (no LLM call): re-score with one scoring the lobsters `files` generate
   (`second_application_in_benchmark-d8d8c92-1`) and its `surface` domains and features replays, read the
   delivery_router `surface` replays (f7ac334, 100%), and write the comparison table in `# Findings`; verified by
   the table and the ledger line.
2. [ ] Paid measures (user's go): delivery_router `just benchmark --provider deepseek --runs 5 --no-confidence`
   once per path (`files`, `surface`, cents), and one lobsters `--domains-from surface --series hidden --runs 1`
   (~$0.5) to set calls, time and judged recall against the `files` run; verified by the tables in `# Findings`
   and the ledger lines. Stop: the decision is the user's.
3. [ ] Apply the decision: the default of `DomainsFrom`, the config `init` writes, a requirement in
   `specs/pipeline-settings.md` replacing its non-goal, an ADR on the default (ADR 0020's "default stays
   `files`" clause), `PLAN.md`, `docs/`, the "default: files" lines of the skills `smoke-test` and
   `quality-benchmark`; verified by a config test traced to the new requirement; makes true SET-n (new).

## Decisions
- Mode: autonomous
- Base commit: 21e4d68
- Unparked (user, 2026-10-28): deliverables 1 to 3.
- Paid budget (user, 2026-10-28): delivery_router 5 runs per path + one `surface` run of lobsters, ~$0.55; no
  linkding confidence or `shown` series.
- No local linkding run (user, 2026-10-28).
- `contexts` stays out of the default: opt-in for large applications, said in the ADR (user, 2026-10-28).

## Deliverable 1
- done: comparison of the saved lobsters runs and reading of the delivery_router replays in `# Findings`; no
  rescore was needed (generate and replays share the scoring of d8d8c92).
- checks: issue file only; `just check` and the code review below.
- red: none (no test, a reading).
- Code review: no findings.
- not verified: the lobsters judge pairs of the replays (not saved); the judge was right on 47 of 49 pairs of
  the generate.
