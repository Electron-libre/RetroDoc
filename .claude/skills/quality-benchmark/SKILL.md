---
name: quality-benchmark
description: Measure the quality of the generated functional docs against a hand-written reference on a public repository (recall and precision of domains and features, business-language narratives, cost), with the repo's own docs hidden then shown, and compare with the previous benchmark. Use when asked to benchmark quality, to know whether a pipeline change makes the docs better, or before and after a step of the redesign (ADRs 0019 to 0023).
---

# Quality benchmark

**Two levels.** To compare a change in one pass (a prompt, a setting, a model for that pass), replay that
pass alone on frozen inputs first: a few cents and a few minutes, without the variation of the passes
upstream (see "Replay one pass" below). The whole-run benchmark that follows stays the final check.

`skill:smoke-test` checks that `generate` is robust; this checks that what it writes is right. The
reference is written by hand per repository in `benchmark/<repo>/reference.yaml` (purpose, actors,
domains, features, a sample of use cases and rules, the pinned commit) and the hand-written pairs in
`benchmark/<repo>/matches.yaml`. Only **public** repositories go in the benchmark.

## Run

```sh
just benchmark benchmark/<repo>/reference.yaml                 # 1 run per series
just benchmark benchmark/<repo>/reference.yaml --runs 3        # the spread needs 3 at least
just benchmark <reference> --model <m> --base-url <u>          # another model or server
just benchmark <reference> --provider deepseek --runs 3 --no-confidence   # hosted: key in .env, about $0.30 per linkding run
just benchmark <reference> --series hidden                     # one series only
just benchmark <reference> --domains-from surface              # the new path of ADR 0020 (default: files)
just benchmark <reference> --writing-temperature 0.7           # another temperature on the writing passes (brief, domains, features, use-cases, rules); they are at 0 by default (ADR 0031)
```

It takes minutes to hours per run (2 series × N runs), so launch it with `run_in_background` and read
the logs in `~/retrodoc-runs/retrodoc-benchmark-<repo>/<timestamp>/<series>/run-<n>.log`. Same local LLM rules as
the smoke test (Ollama on `:11435`, or the faster hybrid llama-server on `:11437`, large context, don't run two at once).

What `benchmark.rs run` does: clones the repository of the reference at its pinned commit (cached in
`~/retrodoc-runs/retrodoc-benchmark-<repo>/source`); for each series, `hidden` (the repo's own docs left out of
`ingest.existing_docs_paths`) then `shown`, and each run, clones afresh, runs `retrodoc init`,
`generate` and `retrodoc benchmark --judge --out run-<n>.json`; then `retrodoc benchmark-table` writes
`table.md` with the mean, range and number of runs of each figure and its change against the previous
benchmark of the same repository.

## Reading the table

The model renames domains and features on every run, so pairs written by hand cover only part of what
a later run generates. The figures are therefore read like this:

- **(judged)** figures: the hand-written pairs, equal names, **and the LLM judge's proposals**. They are
  the main figures. The judge shares the biases of the model that generated the docs, hence the audit
  below.
- **Narratives in business language (judge)** and **use cases fully in business language (score 1)**: the
  two measures to follow. The mean of the deterministic score saturates near 95% and hides differences.
- **no suffix** (strict): the hand-written pairs and equal names only. An audited floor, which rises as
  verified pairs are added to `matches.yaml`.
- A change smaller than the range between runs is noise, not progress. One run is a sample, not a
  measure: use `--runs 3` at least before concluding.

## Audit (after each benchmark)

1. Pick one run out of three per series and read the judge's pairs in
   `~/retrodoc-runs/retrodoc-benchmark-<repo>/clones-<timestamp>/<series>-<n>/.retrodoc/benchmark/judge.yaml`
   (they are also the ones the table counted in `(judged)`).
2. For each pair, check it against `reference.yaml` (descriptions included, not only the names): same
   business capability, or not? Count the false ones: that is the error rate of the judge, to report
   next to the table.
3. Copy the pairs you checked and find right into `benchmark/<repo>/matches.yaml` (the pairs of a
   `domains:` and a `features:` list: `generated` is the model's name, `reference` a name of
   `reference.yaml`). Never copy a pair you did not check. A pair naming an unknown reference name
   makes the next run fail on purpose.
4. A narrative the judge rated as technical is worth reading too: if the rating is wrong, say so in the
   report; there is no file to correct it.

## After the run

- Report the table, the number of runs and what moved beyond the spread.
- A `FAIL` line (generate or benchmark failed in a run) is a finding: read the log around the first
  `WARN`/error. A run that failed is missing from the table, which says so in its `Runs` column.
- Don't commit the clones, logs or tables (they live in `~/retrodoc-runs`); commit reference and match files only.

## Replay one pass (the first level)

The frozen inputs of a repository live in `~/retrodoc-runs/frozen/<repo>/` (`manifest.yaml`: the model that
wrote them, the commit, the run they come from and the origin of the domains, features and use cases that a
replay froze; `cache/`: the artifacts). `freeze_inputs` makes them from a complete run; the replays of
the `use-cases` and `rules` passes read the code of the tree at `manifest.source_run`, so keep that run.

```sh
set -a; . ./.env; set +a
cargo run -q -p retrodoc-pipeline --example pass_replay -- <domains|features|use-cases|rules> \
  ~/retrodoc-runs/frozen/<repo> benchmark/<repo> deepseek-chat 3
```

Replay a pass 3 times at least, once with the setting kept and once with the setting changed, and compare
the means and ranges: a difference inside the range is noise. Domains and features are scored against
`reference.yaml` strictly and with the judge's pairs (the strict figure is near zero, the models rename
everything: read the judge columns); use cases report their number and the business language by the judge and
by the deterministic score; rules report the rules stated and the rules of the reference a generated rule
states. Calls, tokens and seconds of the pass alone come with each. A change in the output of an upstream pass
asks for new frozen inputs (`--freeze-first` on `domains`, `features` and `use-cases`, in that order, rewrites
the frozen artifact of the `surface` path from the first run); say which model wrote them. The judge is the
model of the replay, so it shares its biases: use the figures to compare settings, not as absolute values.
