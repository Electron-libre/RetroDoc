---
priority: medium
why: 'the only benchmark whose reference a human can check item by item; the milestone can be reached without it, by the reading of the issue it complements'
---
# Benchmark the generated docs of Vingilot against its real intent

# Goal
The benchmark references (linkding, Lobsters) were drafted by agents, on applications the user does not know: a
human can check their method, not their items. Vingilot, a virtual tabletop for role-playing games written with
Cardinal, to be public (pushed, private for now), known in detail by the user, and its intent is written down: specs with numbered
requirements, ADRs, closed issues, a product document, a roadmap. Expected: a Vingilot reference built from that
intent and validated item by item by the user, then a `hidden` series (every source of intent left out) and a
`shown` series scored against it, the substance compared and not the form, with the findings dated in
`docs/findings.md`.

# Findings
* 2026-11-13, Vingilot at `7a14fdd`: 572 commits, 15 specs, 16 ADRs, closed issues, `PRODUCT.md`, `ROADMAP.md`,
  `ROADMAP-done.md`, `CHANGELOG.md`, `CLAUDE.md`, `AGENTS.md`; no git remote yet.
* The `hidden` series of `benchmark.rs` only empties `existing_docs_paths`: the file walker still sees `specs/`,
  `issues/`, `CLAUDE.md`, `AGENTS.md`, the roadmap and the changelog if they are not Markdown docs to it, and the
  commit history, the `// spec: ID` comments and the `//!` docs stay. In a project written with Cardinal these
  give the intent away far more than in an inherited project (not measured yet).
* The specs mix functional requirements with technical ones (protocol, storage, serving): a spec is not a feature
  one to one.

# Approach
1. Measure the leak first: list what RetroDoc reads of Vingilot with `existing_docs_paths` empty (`scan`, the
   inputs of the passes), and which of these files or lines carry the intent.
2. Hide the intent in the clone of the `hidden` series: candidates, to combine: (a) delete `docs/`, `specs/`,
   `issues/`, `PRODUCT.md`, the roadmaps, `CHANGELOG.md`, `CLAUDE.md`, `AGENTS.md`, `.claude/` from the clone (a
   list in the benchmark folder of the repo); (b) a third series with the history squashed to one commit, to see
   what the commit messages bring; (c) leave the `// spec:` and `//!` comments and state the bias.
3. Draft `benchmark/vingilot/reference.yaml` from the specs, `PRODUCT.md`, the ADRs and the closed issues (purpose,
   actors, domains, features, a sample of use cases and rules), each feature with its `evidence` (the spec
   requirement or the heading it comes from); keep the functional level only.
4. The user corrects the draft item by item (right, wrong, missing, too technical); record the corrections as the
   checking of the reference.
5. Estimate the cost, get the user's go, then `just benchmark` on the `hidden` and `shown` series; pair the names in
   `matches.yaml`, score, compare with linkding and Lobsters; the user reads the misses and the extras.
6. Write the findings and propose the issues they call for.

# Milestone
Read by someone taking an application over

# Resources
* Skill `quality-benchmark` and `.claude/skills/quality-benchmark/benchmark.rs` (`patch_config`, the series).
* `benchmark/linkding/` and `benchmark/lobsters/` (reference, matches, entry points), spec
  `benchmark-reference-evidence`.
* `issues/read_by_someone_who_knows_the_application.md` (the human reading this measure complements).
* The measures ledger, `docs/findings.md`.

# Hints
* The benchmark clones `repository` of the reference: Vingilot is pushed (2026-11-19), private until the user
  publishes it; the user's account can clone it, and its reference goes in `benchmark/vingilot/` as for a public
  repository (no local path in a committed file).
* Cardinal, RpgAssistant and RetroDoc itself are candidates for later, with two biases to state: four Rust
  tools by the same author and harness, and RetroDoc measured on itself tunes the pipeline to its own docs.
* The user's time is the cost: give them a short reference to correct, not the generated docs to read in full.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-19

## Understanding
Measure how close the generated docs of Vingilot come to its written intent, with a reference the user checks item
by item. Out of scope: fixing the leak of the existing `hidden` baselines (linkding, Lobsters), a spec (a measure).
Means: step 1 of the Approach is done below; option (a) of step 2, (c) for the comments.

Leak measured 2026-11-19 on Vingilot `b73679e0`, `existing_docs_paths = []`:
* `scan`: 998 files, of which 368 Markdown (`specs/`, `issues/`, `docs/adr/`...) walked with the role `docs`.
* `signals::collect` reads every Markdown file of the root whatever `existing_docs_paths` says (`root_markdown`):
  `README.md`, `PRODUCT.md`, `ROADMAP.md`, `ROADMAP-done.md`, `CHANGELOG.md`, `CLAUDE.md`, `AGENTS.md` all feed the
  brief. The same holds for the `hidden` baselines: linkding reads its `README.md` and `CHANGELOG.md`, Lobsters its
  `README.md`, `CLAUDE.md`, `AGENTS.md`, `CONTRIBUTING.md`.
* History: 284 `docs:` subjects of 1,351 commits, 280 subjects naming an issue.
* Code: 1,507 `// spec:` traces and 1,055 `//!` lines.

## Plan
1. [x] Intent hidden in the `hidden` series: `benchmark/<repo>/hidden.txt` lists the paths (files, folders) removed
   from each `hidden` clone before `generate`; no list, no change (the linkding and Lobsters baselines stay
   comparable); verified by a test of `test_benchmark.rs`; ~120 lines. (autonomous)
2. [ ] Vingilot reference: `benchmark/vingilot/reference.yaml` (pinned commit, purpose, actors, domains, features
   with their `evidence` in the specs, a sample of use cases and rules, functional level only) and its `hidden.txt`;
   verified by the `reference_evidence` check; **stop: the user corrects the reference item by item**; ~500 lines.
3. [ ] Measure: estimate, **stop for the user's go**, `just benchmark` on the `hidden` and `shown` series; pairs in
   `matches.yaml`; ledger line; ~100 lines.
4. [ ] Findings dated in `docs/findings.md` (hidden against shown, against linkding and Lobsters, the misses and
   extras the user reads, the root Markdown leak of the baselines) and the issues they call for; ~60 lines.

## Decisions
- Mode: autonomous
- Base commit: 75462129196ef7b5810a8fdff712a596ce1670dc
- History kept in `hidden`, its bias stated (no squashed series).
- Vingilot's `README.md` hidden with the rest.
- The root Markdown leak of the linkding and Lobsters baselines: a finding and an issue of its own.
- One run per series.

## Deliverable 1
done: `benchmark.rs hide-intent` and the `hidden` series remove the paths of `benchmark/<repo>/hidden.txt` from
the clone; the skill and `docs/dev/agent-harness.md` say so and name the root Markdown leak.

| check | result |
|---|---|
| tests (`just test-skill-scripts`) | pass |
| `project-check` | pass |
| docs | skill, agent-harness |
| code review | low, see below |

red: none (no spec: a skill script); the new check failed with `FAIL - removes the listed files and folders from the
clone, and keeps the rest` before the code.
Code review: 1 finding: a path under a listed folder, or listed twice, failed the removal and left the clone half
emptied; fixed (skipped once gone) with its check.
not verified: a real `hidden` run with a `hidden.txt` (deliverable 3).
decisions needed: none.
