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
