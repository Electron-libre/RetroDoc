# Decide whether `domains_from = "surface"` becomes the default

# Goal

Settle the default of `[pipeline] domains_from` (ADR 0020) with enough evidence. `surface` cuts the calls and
the time on a large application but its features were worse on a 9-file library; the default stays `files`
until that gap is understood and a second large repository agrees.

# Findings

* Benchmark of 2026-10-10 (`issues/behaviour_coverage_domains.md`, "Comparison"): on linkding with deepseek,
  `surface` makes 110 calls in 6.4 minutes against 222-283 in 9.7-14.4, with judged feature recall at par and
  better narratives; on `delivery_router` with deepseek, judged feature precision is 45% against 96% and the
  recall of the three runs ranges from 12% to 88%.
* Not measured: linkding with the local model, the `shown` series, the confidence pass on linkding, a second
  large repository.
* The path depends on the entry points found: on `delivery_router` the share of files reached swings between
  44% and 100% from run to run on the old path too.

# Approach

1. Find why the library does worse: read the entry points, domains and features of the three weak runs
   (a library's public API as entry points, ADR 0008; features that group too many or too few entry points).
2. Fix what the reading shows, or accept that a tiny library is outside the target.
3. Run the benchmark with more runs (5) on `delivery_router`, once on linkding with the local model, with the
   `shown` series and with confidence on, and on a second public application.
4. Decide the default; update `PLAN.md`, `CLAUDE.md`, the ADR and the default config written by `init`.

# Resources

* `issues/behaviour_coverage_domains.md` (comparison tables), `benchmark/`, skill `quality-benchmark`
  (`--domains-from surface`), ADRs `0008` and `0020`.

# Hints

* "Previous" in the benchmark table is the last benchmark of the repository whatever its model or path:
  compare like with like by hand.
* The local linkding run takes 1 to 2 hours; paid runs need an explicit go.
