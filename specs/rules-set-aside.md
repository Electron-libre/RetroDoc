---
id: RSA
status: draft
adrs: [0021]
---

# Putting back what the rules pass set aside

## Intent

The rules pass sets aside, on a judgement of the LLM, the features it takes for tests (they get no rule) and
the rules it drops as noise. The user who reads the debt report and finds a wrong case must be able to correct
it once, in `retrodoc.toml`, and have the correction hold across reruns and `generate --force`, without the
LLM being asked again for the whole repository.

## Requirements

- **RSA-1** (deterministic) A feature whose slug is listed in `keep_features` of the `[rules]` section of
  `retrodoc.toml` must not be asked whether it is about tests: its rules must be stated and judged as for
  any other feature.
- **RSA-2** (deterministic) Adding a feature to `keep_features`, or removing it, must redo that feature only:
  the other features make no LLM call.
- **RSA-3** (deterministic) A rule dropped by the judgement whose slug is listed in `keep_rules` of the
  `[rules]` section must be published as if the judgement had kept it, with its evidence, and with no LLM
  call when the feature is otherwise unchanged.
- **RSA-4** (deterministic) Removing a slug from `keep_rules` must set that rule aside again on the next run,
  with no LLM call when the feature is otherwise unchanged.
- **RSA-5** (deterministic) A slug of `keep_features` that names no feature of the run, or a slug of
  `keep_rules` that names no rule set aside nor published by the run, must give one warning naming the slug
  and its key; the run must go on.
- **RSA-6** (deterministic) `generate --force` must keep both lists: they live in `retrodoc.toml`, not in
  `.retrodoc/cache/`.
- **RSA-7** (deterministic) The debt report must say, at each list of what was set aside, which key of
  `retrodoc.toml` puts an entry back, and must mark the rules put back by hand.
- **RSA-8** (deterministic) A listed rule set aside by a run that did not keep the dropped rule whole (a
  `rules-set-aside.yaml` saved before this capability) must give a warning saying it comes back when its
  feature is stated again; no rule must be made up from the statement alone.

## Scenarios

- (RSA-1) Given a feature `email-verification` listed in `keep_features` and an LLM that would answer
  "about tests", when the rules pass runs, then the kind question is not asked and the feature has rules.
- (RSA-3) Given a run where `orders-2` was dropped as `storage`, when `orders-2` is added to `keep_rules` and
  `generate` runs again, then `rules.yaml` holds `orders-2` with the evidence it was stated with, and no
  LLM call is made.
- (RSA-5) Given `keep_rules = ["orders-9"]` and no rule `orders-9`, when the pass runs, then one warning
  names `orders-9` and `keep_rules`.

## Non-goals

- A rule is named by its slug `<feature-slug>-<n>`: when its feature is stated again (changed code, model or
  prompt), the numbering may change and the slug then names whatever rule bears it. The rules are not
  tracked by their content; the report marks what is put back, so the user can check it.
- No way to set aside by hand a feature or a rule the LLM kept.
- Whether the LLM judges right is not required here (`issues/done/rules_deduplication_and_noise.md`).
