---
priority: medium
why: 'the only benchmark whose reference a human can check item by item; the milestone can be reached without it, by the reading of the issue it complements'
---
# Benchmark the generated docs of Vingilot against its real intent

# Goal
The benchmark references (linkding, Lobsters) were drafted by agents, on applications the user does not know: a
human can check their method, not their items. Vingilot, a virtual tabletop for role-playing games written with
Cardinal, is public (soon pushed), known in detail by the user, and its intent is written down: specs with numbered
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
* The benchmark clones `repository` of the reference: Vingilot must be pushed first (no local path in a committed
  file).
* Cardinal, RpgAssistant and RetroDoc itself are candidates for later, with two biases to state: four Rust
  tools by the same author and harness, and RetroDoc measured on itself tunes the pipeline to its own docs.
* The user's time is the cost: give them a short reference to correct, not the generated docs to read in full.
