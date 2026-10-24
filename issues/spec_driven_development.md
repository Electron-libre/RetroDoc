# Introduce spec-driven development with a traced `specs/` directory

# Goal
Make the expected behaviour of each capability a durable, checkable document that comes before the code. Today
the "what" is scattered: `PLAN.md` (roadmap), the ADRs (why a choice was made), the `//!` doc of each pass and
the issues (short-lived). Nothing says, per capability, what must hold for it to be correct, and nothing links
such a statement to the test that proves it. The expected outcome: a root `specs/` directory with numbered
requirements, a script that fails `just check` when a requirement and its tests drift apart, and a workflow
where a change of behaviour starts with the spec.

# Approach
Decisions already taken by the user: specs live in a root `specs/`; traceability is a `// spec: ID` comment in
the tests, checked by a script; specs are written capability by capability when one is touched, no large
retro-documentation. Proposed deliverables, in this order:
1. Template and `specs/README.md`, plus one pilot spec on the incremental rerun (ADR 0005). A spec holds: intent,
   requirements with stable ids (`RER-1`, `RER-2`, ... "must" / "must not"), Given/When/Then scenarios for edge
   cases, non-goals, links to ADRs, a status (`draft`, `accepted`, `implemented`). Each requirement declares its
   kind: `deterministic` (proved by a test with a fake `LlmProvider`) or `quality` (proved by the
   `quality-benchmark` skill, with a threshold). The README states the border with ADRs: the spec says what is
   expected, the ADR says why it was chosen.
2. A traceability check in `rust-script` (like the other harness scripts, tested by `just test-harness`), wired
   into `just check`: an `implemented` deterministic requirement with no `// spec: ID` test fails; a test citing
   an unknown id fails; a `quality` requirement must name its benchmark.
3. Adapt `skill:issue-workflow` (the spec becomes a validation stop before the plan: write or amend the spec,
   get it validated, then write the tests from it), `skill:create-issue` (a `Specs` section and its checker)
   and the `CLAUDE.md` index (read the spec of a capability before touching it); extend `skill:update-docs`.
4. A skill `write-spec` that adds or amends a spec with the template and validates its format, on the model of
   `create-issue` and its checker.
5. Progressive extension: one spec per capability, written when the capability is next changed.

# Resources
* `docs/adr/0005-incremental-rerun-fingerprints-and-resume.md`, `docs/crates/pipeline.md` (the pilot's source)
* `.claude/skills/issue-workflow/`, `.claude/skills/create-issue/` and its `check_issue.rs`, `.claude/skills/test_skills.rs`
* `docs/adr/0014-agent-harness-hooks-skills-rust-script.md`, `docs/dev/agent-harness.md`, `justfile`
* `.claude/skills/quality-benchmark/` for the `quality` requirements

# Hints
* Some behaviour is not deterministic (LLM output): a requirement that cannot be proved by a test with a fake
  provider must be `quality`, not left untraced.
* Keep ids stable once a spec is `accepted`: tests and issues cite them.
* Specs are in English like the rest of the repo and name no confidential test repository.

# Tracking
1. [x] Format and pilot spec: `specs/README.md`, `specs/TEMPLATE.md`, `specs/rerun.md` (`draft`, prefix `RER-`), ADR 0032. Checked by mapping each requirement to the existing test that proves it.
2. [x] Traceability check: `check_specs.rs` and its test, `just check-specs` in `just check`, `test-harness` extended; annotate the pilot's tests and move `rerun.md` to `accepted`. Checked by `just test-harness` and `just check`.
3. [ ] Skill `write-spec` (template, validation by `just check-specs`), structure checked by `test_skills.rs`.
4. [ ] Adapt `issue-workflow` (spec stop before the plan), `create-issue` (`Specs` section and checker), `CLAUDE.md`, `update-docs`, `docs/dev/agent-harness.md`.

Step 5 of the approach (one spec per capability, written when it is next changed) is not a deliverable: it is how the project goes on.

## Decisions
* Specs live in a root `specs/`; traceability is a `// spec: ID` comment in tests, checked by a script.
* `check-specs` is part of `just check` (makes `rust-script` a requirement of the pre-commit gate).
* A `quality` requirement names `benchmark: benchmark/<repo>/reference.yaml` and `metric:`, with an optional threshold; the script only checks the file exists, it does not rerun the benchmark. The pilot has none.
* `implemented` is set by hand by whoever delivers the last requirement; only `accepted` and `implemented` specs must have every `deterministic` requirement traced, `draft` is exempt.
* `write-spec` comes before the workflow adaptation so the workflow cites an existing skill.
* The check scripts live in `.claude/specs/`, not in `.claude/skills/write-spec/` (`test_skills.rs` requires a `SKILL.md` per skill dir, which arrives with deliverable 3).
* `withdrawn:` must start the statement of a withdrawn requirement; a `// spec:` comment must sit right above a `fn` (attributes and comments allowed between).
