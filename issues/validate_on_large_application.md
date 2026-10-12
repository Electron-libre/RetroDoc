# Validate the redesigned pipeline on a large real application

# Goal
Know whether RetroDoc now reaches its product goal on its real target: a large legacy application whose
functional documentation a reader who does not know the code can use. Every measure since the redesign
(ADRs 0019 to 0028) comes from two small public repositories; the last complete `generate` on a large
application predates the redesign. One complete run, read by the user, says whether the direction holds
and gives the evidence the parked decision on `domains_from` is waiting for.

# Findings (project review of 2026-10-12)
* The only complete `generate` on a large application is of 2026-10-02 (`PLAN.md` §7.2): a sparse checkout
  of 325 source files of a private Rails application, local `qwen3.6:35b-a3b`, about 2h20, 7 domains, 77
  features, 183 use cases, 13 features without a use case. It ran the `files` path, before the brief, the
  business files, the structured outputs, the mechanical confidence, tree-sitter and the rules.
* On the cache of an older run of that application, 36% of its 1,661 source files are reached by a
  behaviour (`issues/done/behaviour_coverage_domains.md`); nobody has read what the other 64% are.
* The `surface` path is measured on linkding (178 source files, deepseek: 110 calls and 6.4 minutes against
  222-283 and 9.7-14.4) and on a 9-file library. Not on a repository of thousands of files.
* The local model is about ten times slower than the hosted one on linkding (86-118 minutes against
  10-14): the whole application is out of reach locally in one sitting.
* The application is confidential: sending its code to a hosted provider is the user's decision.

# Approach
1. Decide with the user: which application, hosted model or local, whole repository or a bounded part
   (`--max-files`, sparse checkout), `files`, `surface` or both, rules on or off. Estimate the cost first
   from the tokens per source file of the linkding runs (`usage.json`).
2. Run it with the smoke-test skill's criteria (finishes, warnings, rerun is a no-op, report) and keep the
   recap per pass: calls, tokens, time, unparseable answers, skipped units.
3. Read the result by hand with the user, on a sample fixed before the run: domain names, 20 use cases
   picked evenly, the rules of 3 features, the entry points without a feature and the technical support
   list. For each: right, wrong, technical noise, missing.
4. Write the figures and the reading in this issue, generically (no name, path or code of the application),
   and turn each defect into an issue or a line of an existing one.
5. Give the result to `issues/surface_domains_default_decision.md`.

# Resources
* `PLAN.md` §7.2 and §7.5, `issues/done/behaviour_coverage_domains.md`
* `issues/surface_domains_default_decision.md`
* Skill `smoke-test`; `crates/retrodoc-pipeline/examples/behaviour_baseline.rs`
* `CLAUDE.md` (confidential repositories)

# Hints
* Never name the application, nor quote its paths or code, in a committed file.
* A paid run needs an explicit go from the user, with the estimate.
* One run is one sample: it validates the direction and finds defects, it does not compare two settings.

# Tracking

1. [x] Preparation without LLM call: clone in `/tmp`, config, `scan`, reading sample fixed.
2. [ ] The paid run: `generate`, rerun, `report`, verdict of the smoke-test criteria, `behaviour_baseline`;
   figures per pass written here.
3. [ ] Reading with the user on the fixed sample; the reading written here, generically.
4. [ ] Each defect to an issue or a line of an existing one; result given to
   `issues/surface_domains_default_decision.md` and `PLAN.md` §7.6.

## Decisions

* (user, 2026-10-12) The private Rails application of the earlier smoke tests, at its committed `HEAD`:
  1,661 source files, 670 test files (`scan`).
* (user, 2026-10-12) Hosted model (DeepSeek), `surface`, rules off, one run. A ceiling of $15 was refused as
  far too much at this stage; the order of magnitude accepted is $1 to $2.
* The smoke script only targets the local server: the same steps are run by hand and judged with
  `smoke.rs evaluate`.

## Reading sample (fixed before the run)

* every domain name;
* 20 use cases: the list sorted by identifier, one every `N / 20`, starting at the first;
* the entry points no feature holds, and the technical support list (the first 30 of each if longer);
* no rules (the pass is off).

## Findings while preparing

* `--max-files` does not bound the `surface` path: `generate` applies the budget after the surface passes
  (roles, business files, glossary, entry points) and the actors, and the features of the `surface` path are
  built from every entry point, kept file or not. The budget only removes files from the code slices the use
  cases read and from the coverage figures. To record as a defect (deliverable 4).
