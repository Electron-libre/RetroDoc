# Let the user put back a feature or a rule the rules pass set aside

# Goal
The rules pass sets aside, on a judgement of the LLM, the features it takes for tests (they get no rule) and
the rules it drops as noise. The trace is kept and listed in the debt report, but nothing lets the user say
"this one is wrong": a business feature taken for tests stays without rules until its text, its model or the
prompt changes. Give a way to correct a doubtful case that survives the reruns, without re-asking the LLM
for the whole repository.

# Findings (deliverable 5b of `issues/done/rules_deduplication_and_noise.md`, 2026-10-23)
* The question "is this feature about tests?" was right for the 30 features of a public repository (9 yes, no
  error) and 0 of 85 on a large application, on the name and description alone. No false yes was seen, but a
  feature such as one that verifies an email address could be taken for tests, and loses every rule.
* The report lists the 14 features and 34 rules set aside on that repository; reading them, a few rule drops
  are debatable (the re-indexing of the other bundles when one is deleted).
* `rules-set-aside.yaml` is a cache file removed by `generate --force`: nothing in it survives a forced run.

# Approach
1. Measure first how often the doubtful cases are real: read the set-aside lists of two more repositories
   (one with a business feature whose name speaks of checking) and count the wrong ones.
2. Candidates, can be combined: (a) a list in `retrodoc.toml` of features to keep (by slug), read before the
   question; (b) a hand-edited file like `roles.yaml` or `product.yaml`, reused until `--force`; (c) the same for
   a rule dropped as noise, by slug, put back from `rules-stated.yaml`.
3. The report says how to put one back, next to each entry.

# Resources
* `crates/retrodoc-pipeline/src/rules/` (`aside.rs`, `judge.rs`, `mod.rs`), `report.rs` (`with_set_aside`)
* `issues/done/rules_deduplication_and_noise.md` (measures, decisions)

# Hints
* Feature slugs change when the features are derived again: a hand list must say what happens to a slug that
  no longer exists (a warning, as for other hand-edited files).
* Priority is moderate: the rules pass is off by default; the trace already shows the risk.
