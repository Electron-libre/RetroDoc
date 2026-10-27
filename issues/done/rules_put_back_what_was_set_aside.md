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

# Specs
* `specs/rules-set-aside.md` (RSA)

# Tracking

Plan: validated

## Understanding
Let the user correct, in `retrodoc.toml` (`[rules] keep_features`, `keep_rules`), a feature taken for tests or a
rule dropped as noise; the correction survives reruns and `--force`, and costs at most the calls of the feature
concerned (none for a rule: the dropped rule is kept whole in `rules-set-aside.yaml`). Out of scope: setting aside by hand.

## Plan
1. [x] Measure: `--rules` with DeepSeek on two more repositories, read their set-aside lists, count the wrong cases; figures here; verified by the figures recorded.
2. [x] Features: `[rules] keep_features` in `retrodoc.toml`, a listed feature skips the tests question, the list is in its fingerprint, unknown slug warned; verified by pipeline tests with a fake `LlmProvider` (calls counted); makes true RSA-1, RSA-2, RSA-5 (features), RSA-6.
3. [x] Rules: the dropped rule kept whole in `rules-set-aside.yaml`, `[rules] keep_rules` puts it back at publishing (no call), removal sets it aside again, unknown or old-format slug warned; verified by pipeline tests with a fake `LlmProvider`; makes true RSA-3, RSA-4, RSA-5 (rules), RSA-8.
4. [x] Report: the key to use at each set-aside list, rules put back marked; docs (`docs/crates/pipeline.md`, README config); verified by a report test; makes true RSA-7.

## Plan (added after the high review)
5. [x] Fixes of the `high` review of the whole issue (see the last section).

## Decisions
- Mode: autonomous
- Base commit: ab5c3b631cd42327c898d3024a6b716c61fe10b3
- The corrections live in `retrodoc.toml` `[rules]` (`keep_features`, `keep_rules`), not in the cache: they survive `--force` and are versioned.
- Measure on `delivery_router` and one public repository with a business feature about checking, with DeepSeek.
- Built even if the measure finds no wrong case: cheap, and the risk is already shown in the report.
- Report: one line per set-aside list naming the key, not a hint on each entry.
- Order: deliverable 1 (measure) waits for a `DEEPSEEK_API_KEY`, absent from the environment of the agent; 2 to 4 do not depend on it (decision above) and go first.
- The lists reach the pass through `UseCaseContext::put_back` (like `rule_evidence`), not a new argument of `build_rules`.

## Deliverable 2: keep_features

done: `[rules] keep_features` (`RulesConfig`) read by the rules pass: a listed feature skips the tests question; only a listed feature carries it in its fingerprint; an unknown slug is warned about.

| check | result |
|---|---|
| tests | pass (3 new in `rules/tests.rs`, 1 in `config.rs`) |
| lint | pass |
| specs | RSA-1, RSA-2, RSA-5 (features), RSA-6 traced |
| docs | `docs/crates/pipeline.md`, `docs/ARCHITECTURE.md`, `README.md`, `rules/mod.rs` |
| code review | see below |

red: `assertion left == right failed: left: 0, right: 1` (the listed feature had no rules: still taken for tests).

Code review: one finding, `keep_rules` is read by nothing: planned, it is deliverable 3 (next).

not verified: a real run (no LLM run in this deliverable).

decisions needed: none.

## Deliverable 3: keep_rules

done: a rule dropped as noise is kept whole in `rules-set-aside.yaml` (`AsideRule::rule`); `[rules] keep_rules` publishes it among the rules of its feature with no call, not as stated, so unlisting sets it aside again; unknown slug and older set-aside file warned about.

| check | result |
|---|---|
| tests | pass (3 new in `rules/tests.rs`) |
| lint | pass |
| specs | RSA-3, RSA-4, RSA-5 (rules), RSA-8 traced |
| docs | `docs/crates/pipeline.md`, `README.md`, `rules/aside.rs` |
| code review | see below |

red: `assertion left == right failed: left: ["orders-2"], right: ["orders-1", "orders-2"]` (the listed rule not published).

Code review: no findings.

not verified: a real run (no LLM run in this deliverable).

decisions needed: none.

## Deliverable 4: report

done: each set-aside list of the debt report names the key that puts an entry back (`keep_features`, `keep_rules`), and a rule put back is marked `put back by hand`; spec RSA set `accepted`.

| check | result |
|---|---|
| tests | pass (1 new in `report.rs`) |
| lint | pass |
| specs | RSA-7 traced; every RSA requirement traced, spec `accepted` |
| docs | `docs/crates/pipeline.md` |
| code review | see below |

red: the assertion on the `keep_features` hint line failed (no hint in the section).

Code review: no findings.

not verified: a real run (no LLM run in this deliverable).

decisions needed: none.

## Deliverable 5: fixes of the high review (`ab5c3b6..c1652e7`)

done: the rules put back are inserted by `with_put_back` wherever `rules.yaml` is written (partial saves included), once per set-aside rule, by their number (`BusinessRule::number_in`, shared with `belongs_to_feature`; the global sort is gone); a feature of `keep_features` is no longer listed as set aside, even when its answer is unreadable; the report marks a rule put back only when `rules.yaml` holds it.

| check | result |
|---|---|
| tests | pass (3 new in `rules/tests.rs`, report test extended) |
| lint | pass |
| specs | no change (RSA-3, RSA-7 hold more strictly) |
| docs | `docs/crates/pipeline.md` |
| code review | see below |

red: `a_rule_listed_twice_is_put_back_once` (two `orders-1`), `a_failing_run_keeps_the_rules_put_back_in_what_it_saves` (`orders-1` missing from `rules.yaml`), `a_feature_put_back_whose_answer_is_unreadable_is_no_longer_listed_as_set_aside` (1 feature left).

High review findings: 8; fixed 7 (report mark, slug listed twice, partial saves, carried feature, global sort, its cost, duplicate slug parser). Not fixed: `AsideRule` keeps `slug` and `statement` beside the whole rule, on purpose: an older file has only them, and the report and a reader of the YAML read them without the nested rule.

Code review (low, on these fixes): no findings.

not verified: a real run.

decisions needed: none.

## Deliverable 1: measure (DeepSeek `deepseek-chat`, 2026-10-27)

Full `generate` with `[pipeline] rules = true`, `domains_from = "surface"`, after deliverables 2 to 5 (nothing
listed in `[rules]`). The public repository is healthchecks (Django, about 21k lines of Python outside the
tests), chosen because its business features speak of checks: "Check Management", "Service health check".
Runs in `~/retrodoc-runs/rsa-dr` and `rsa-hc`; set-aside lists read by hand by the assistant.

| | `delivery_router` | healthchecks |
|---|---|---|
| features / taken for tests | 11 / 0 | 38 / 0 |
| rules published / dropped as noise | 39 / 1 | 346 / 52 (34 technical, 17 screen, 1 storage) |
| clearly wrong drops (a business rule lost) | 0 | 0 |
| debatable drops | 0 | 3 |
| rules pass | 33 calls, 17 s | 114 calls, 1m39s (one feature cut and skipped, retried next run) |
| whole run | | 268 calls, 1.12M tokens in, 127k out, 6m50s |

* The features named after checks were not taken for tests: with the linkding and large-application figures,
  0 wrong answers out of 134 features, still no false yes.
* The 3 debatable drops on healthchecks: a new API key revealed only on click (`screen`, a security choice), a
  pause asking a second confirming click (`screen`), the project last opened remembered (`storage`). The rest
  is display formatting, retries, statsd, documentation build, favicons, as intended.
* Conclusion: the doubtful cases are rare (3 of 53 entries, none clear) but they exist, and the correction
  costs no LLM call for a rule: deliverables 2 to 4 stand as built (decision "built even if the measure finds
  no wrong case").
