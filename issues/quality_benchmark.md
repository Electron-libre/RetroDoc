# Measure the quality of the generated functional docs against a hand-written reference

# Goal

Know whether a pipeline change makes the documentation better, not only whether the run ends without a
warning. The smoke test (`.claude/skills/smoke-test/`) checks robustness: `generate` finishes, no `WARN`,
the rerun is a no-op, the report exists. Nothing measures whether the domains, features, use cases and
rules match what the product really does. The redesign proposed in ADRs 0019 to 0023 needs that
measure before and after each step.

# Findings

* Quality has only been judged by hand, once per run, in `PLAN.md` §7 (domain names, actors, narratives
  on the Rails test repository), and the model answers differently on each run.
* The validation criterion of `PLAN.md` §7.1 is "a reader who doesn't know the code should be able to say
  what the product does", which is not reproducible as stated.

# Approach

1. Pick two or three repositories of different shapes: `delivery_router`, and one or two open-source
   applications of medium size whose user documentation is good (a web application with a public user
   guide). Pin a commit of each.
2. Write the reference by hand (or from their user docs) for each: purpose in two sentences, actors,
   domains, features, a sample of use cases and business rules.
3. Run `generate` with the user docs hidden (left out of `existing_docs_paths`), then with them, and
   score: recall and precision of domains and features against the reference (matched by hand or by an
   LLM judge with the reference in the prompt), share of narratives a judge rates as business language,
   number of calls and tokens (`usage.json`).
4. Run each configuration several times (three at least) and report the spread, not one sample.
5. Make it a `just` recipe and a skill, like `smoke-test`, that writes a small table to compare with the
   previous run.

# Resources

* `.claude/skills/smoke-test/`, `justfile`
* `PLAN.md` §7 (past manual judgments), `crates/retrodoc-pipeline/src/usage_log.rs`
* ADRs 0019 to 0023

# Hints

* Don't commit the clones or the run logs; keep the references (small YAML or Markdown) in the repository.
* Repositories used for smoke tests may be confidential: only public ones go into the benchmark.
* A judge LLM shares the biases of the generating one: keep part of the scoring manual, at least at first.

# Tracking

1. [x] Reference format (`benchmark/<repo>/reference.yaml`) and deterministic metrics collected from a run's artifacts and `usage.json`
2. [x] Matching file (`matches.yaml`, manual, takes precedence) and recall/precision of domains and features
3. [x] LLM judge, as a `retrodoc benchmark` subcommand: proposes matches for what `matches.yaml` leaves out, rates narratives as business language
4. [x] Orchestration: `just benchmark <repo>`, N runs with user docs hidden then shown, table with mean and spread, comparison with the previous run
5. [x] References (`delivery_router` drafted by the agent and corrected by the user, then `linkding`), first real runs, `quality-benchmark` skill, docs
   * done: both references and `delivery_router` pairs committed, skill and docs written, `just benchmark` run end to end on `delivery_router` (3 runs per series)
   * done too: baseline of `delivery_router` and audit of the judge's pairs (see Baseline)
   * left, outside this checklist: first `linkding` measure (too slow for the 2 h background limit, see Decisions)

## Decisions

* Repositories: `delivery_router` and `linkding` (public ones only, commit pinned when the reference is written). Not `errbit` (instrumentation component) nor `popcorn-nantes` (static site).
* LLM: the local one (`qwen3.6:35b-a3b`, see the smoke-test skill).
* Judge: an LLM with the reference in its prompt; `matches.yaml` (manual) takes precedence over it.
* The judge lives in the CLI as `retrodoc benchmark`, to reuse the provider and `FakeLlm`.
* The agent drafts the `delivery_router` reference, the user corrects it. The `linkding` reference is not written without the user's review.
* Start with 1 run per configuration to validate the mechanics, then 3 (spread to report).
* The judge uses the model of the clone's `retrodoc.toml` (the generating one). A `--judge-model` option is left for later, if comparing judges becomes useful.
* The `--judge` path of `retrodoc benchmark` is not exercised end to end yet (only the modules are tested with `FakeLlm`): check it on the first real run (deliverable 5).
* Aggregation lives in Rust (`retrodoc benchmark-table`, tested with `cargo test`); the `just benchmark` script (`.claude/skills/quality-benchmark/`) only orchestrates. Clones are kept in `/tmp` for inspection, with no automatic cleanup.
* `benchmark.rs run` has not run end to end yet (no reference to give it): check it on the first real run (deliverable 5).
* `linkding`: its reference is committed but not measured yet. One `generate` of the 442 files took about 1h40 locally (the run was stopped by the 2 h background limit at 85% of the use cases pass), so a benchmark needs a terminal without limit, or a faster model (`--model`, `--base-url`). Only `delivery_router` is benchmarked for now.
* The judged figures are the main ones, the strict ones an audited floor (the model renames domains and features on every run, so hand-written pairs only cover part of a run); an audit of a sample of the judge's pairs follows each benchmark (see the skill).

## Baseline

`delivery_router` at its pinned commit, local `qwen3.6:35b-a3b`, 3 runs per series, before the redesign of ADRs 0019 to 0023 (mean and range):

| Figure | docs hidden | docs shown |
|---|---|---|
| Features recall, audited | 29% (12-38%) | 38% (38-38%) |
| Features precision, audited | 43% (20-60%) | 62% (50-75%) |
| Domains recall, audited | 67% (50-100%) | 50% (50-50%) |
| Domains precision, audited | 89% (67-100%) | 100% (100-100%) |
| Domains recall, judged | 67% (50-100%) | 50% (50-50%) |
| Features recall, judged | 46% (38-50%) | 46% (38-50%) |
| Features precision, judged | 76% (67-80%) | 74% (67-80%) |
| Narratives in business language (judge) | 54% (22-71%) | 54% (38-62%) |
| Business-language score (deterministic) | 88% (74-96%) | 93% (91-97%) |
| LLM calls | 23 (21-25) | 21 (19-24) |

* "Audited": the judge's pairs of the six runs were read against the descriptions of the reference (not the code) and the right ones copied to `matches.yaml`. Of 23 feature pairs, 10 were wrong (43%) and 3 were re-paired by hand; the 7 domain pairs held. The judge therefore overstates feature figures: recall by 8 to 17 points, precision by 12 to 33. The audit was done by the same family of model that wrote the code, from descriptions only: a human spot check of `matches.yaml` is still due.
* The spread is as large as most of the changes seen between single runs (a first benchmark with one run showed +50 points of domain precision that three runs do not confirm): never conclude from one run.
* The two measures of business language disagree (judge about 54%, deterministic score 88 to 93%): to settle by reading narratives by hand.
