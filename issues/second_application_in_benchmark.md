# Add a second public application to the benchmark

# Goal
Know whether a setting that wins on linkding wins elsewhere. The replays of `issues/done/benchmark_passes_on_fixed_inputs.md`
discriminate on one repository only: `delivery_router` (9 files) is too small, its figures saturate. A second
public application, with a reference a human has read, makes a comparison of settings worth believing;
`delivery_router` stays as a smoke repository.

# Findings (replays of 2026-10-23, DeepSeek `deepseek-chat`, 3 runs on frozen inputs)
* `delivery_router`: 2 domains, 5 features, 5 use cases; the judge finds 100% of the reference domains, the
  deterministic business-language score is 100%; a replay of every pass costs under 0.01 EUR. Nothing there
  separates two settings.
* linkding: 8 to 9 domains, 33 to 35 features, 78 to 80 use cases, 301 to 344 rules; the replay of every pass
  costs about 0.3 EUR. The spread between runs is 15 to 20 points on the judged recall, so a gain under that
  is noise.
* A reference is work: the one of linkding has 24 features and 29 rules, drafted by an agent and checked
  against the code by a subagent, not yet read in full by a human.

# Approach
1. Choose the application: public, 100 to 300 source files, real business logic (not a library), a stack
   other than linkding's (Python/Django): Node, Java or Rails. Check its size and licence first.
2. Pin a commit, write `benchmark/<repo>/reference.yaml` (purpose, actors, domains, features, a sample of use
   cases and rules) and `entry-points.yaml`, with the checks used for linkding (each item confirmed in the code;
   quote YAML strings that hold ` #`).
3. One complete `generate` run, frozen with `freeze_inputs`, then `pass_replay <pass> ... --freeze-first` for
   domains, features and use cases (see skill `quality-benchmark`); read the judge's pairs and copy the right
   ones into `matches.yaml`.
4. Say in the skill `quality-benchmark` and in `docs/dev/commands.md` that `delivery_router` is a smoke
   repository only.

# Resources
* `issues/done/benchmark_passes_on_fixed_inputs.md`, skill `quality-benchmark`, `benchmark/linkding/`
* `crates/retrodoc-pipeline/examples/freeze_inputs.rs`, `examples/pass_replay.rs`

# Hints
* The frozen inputs must name the model that wrote them (`manifest.yaml`); the replays of the use cases and
  rules read the code of `manifest.source_run`, keep that run.
* Only public repositories go in `benchmark/` (`CLAUDE.md`).

# Tracking

Plan: validated

## Understanding
Give the benchmark a second public application, other stack than linkding, with a reference checked in its code,
frozen inputs and audited judge pairs, so a setting is compared on two repositories. No change of pipeline
behaviour (no spec). Candidate: Lobsters (Rails, BSD-3-Clause, 131 Ruby files and 121 ERB views under `app/` and
`lib/`, 47 models, 46 controllers: stories, comments, votes, moderation, invitations, messages, hats).

## Plan
1. [x] Reference: `benchmark/lobsters/reference.yaml` (pinned commit, purpose, actors, domains, features, a sample
   of use cases and rules) and `entry-points.yaml`, each item confirmed in the code; verified by a test that loads
   and checks every committed `benchmark/*/reference.yaml` and `matches.yaml` (`Reference::load`, `check`).
2. [ ] Measure (paid, user's go): one complete DeepSeek `generate` of the pinned commit, kept as `source_run`,
   `freeze_inputs` into `~/retrodoc-runs/frozen/lobsters`, then `pass_replay --freeze-first` 3 runs each for
   domains, features, use cases; audit the judge's pairs into `benchmark/lobsters/matches.yaml`; ledger lines
   (measures, artifacts, G5 ticked, G2/G3 extended to two applications).
3. [ ] Docs: skill `quality-benchmark` and `docs/dev/commands.md` say `delivery_router` is a smoke repository
   only and name the two benchmark applications; verified by `just check` and `just test-harness`.

## Decisions
- Mode: autonomous
- Base commit: a7487c210d4c000e2273bc822ac88bc81487a6e7
- Application: Lobsters (Rails, BSD-3-Clause), github.com/lobsters/lobsters.
- Paid measure: approved, deepseek-chat, about $1 in total (generate about $0.5, use cases replays about $0.4).
- Reference: checked item by item in the code by the agent, then a stop for the user to read it before the measure.

## Deliverable 1 summary
- done: `benchmark/lobsters/reference.yaml` (8 actors, 7 domains, 33 features, 35 rules, each read in the code of
  ceb5e71) and `entry-points.yaml` (49 files, 24 groups, 8 actors), plus `tests/benchmark_files.rs` that loads and
  checks every committed reference, pair and entry points file.
- red: `left: ["delivery_router", "linkding"] right: ["delivery_router", "linkding", "lobsters"]`.
- not verified: the reference is not yet read by the user (stop before deliverable 2, Decisions); no
  `matches.yaml` yet (deliverable 2).
