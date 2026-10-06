---
name: issue-workflow
description: Drive an issue file from `issues/` to done in validated deliverables (reformulate, plan, then per deliverable test, code, docs, review, human validation, issue update, commit). Use when the user hands over an issue file (e.g. `issues/foo.md`) or says to start, continue or resume an issue, so the process doesn't have to be re-explained in each prompt.
---

# Work an issue

The user hands over an issue file in `issues/`. Talk to the user in **French** (skills, code, docs and
commits stay in English). Never skip a validation stop: they are what makes the autonomy safe.

## 0. Resume or start

Read the issue file. If it has a `# Tracking` section, the issue is already planned: summarize where it
stands (next unchecked deliverable) and continue at step 3. Otherwise start at step 1.

## 1. Reformulate and validate (STOP)

- Read the issue, plus the resources it names (docs, `git log`, past conversations, `PLAN.md`, memory).
- Restate the need in your own words: goal, means, constraints, what is out of scope, open questions.
- Ask the questions you can't answer from the repo. Don't ask what has a conventional default.
- **Stop and wait for the user to validate the understanding.**

## 2. Plan the deliverables (STOP)

- Split the work into small deliverables, each independently testable and committable, ordered by value
  and dependency. Give your recommended order and why.
- For each: what it does, how it will be verified.
- **Stop and wait for the user to validate the plan.** Then write it in the issue under `# Tracking`
  (checklist `1. [ ] …`, plus a `## Decisions` list for the answers given).

## 3. For each deliverable, in order

1. **Gather**: read the code and docs it touches; ask about missing information (one batch of questions).
2. **Test first**: write a behavior or end-to-end test that fails for the right reason. Follow the test
   conventions in `CLAUDE.md` (fake `LlmProvider`, `tempfile::tempdir()`, no network). For non-Rust
   deliverables (hooks, skills, scripts) write the script test in Rust (`rust-script`, no bash) and run it with `just test-harness`.
3. **Implement** until the test passes. The clippy gate hook keeps the code warning-free; fix what it reports.
4. **Docs**: run `skill:update-docs`. If the change is an architectural decision, also write an ADR in
   `docs/adr/` (template and numbering in `docs/adr/README.md`) and update the
   Mermaid diagrams in `docs/ARCHITECTURE.md` when the flow changed.
5. **Review**: run `just check` (fmt, clippy, tests) and `just test-harness`, then `/code-review`
   on the diff; reread the docs you changed. Fix what the review finds.
6. **Human review (STOP)**: summarize what was done, what was verified and **what was not** (state it
   plainly), decisions needed. Wait for the user's validation. Don't commit before it.
7. **Track**: tick the deliverable in the issue's `# Tracking` and note decisions or follow-ups.
8. **Commit**: after validation, run `skill:commit-message` (message per `AGENTS.md`, validated by its
   checker). Commit only the files of this deliverable (never `git add -A`; leave unrelated or personal
   files such as `settings.local.json`) and report the hash. If the user wants to commit themselves, output
   just the message.

Then move to the next deliverable. Don't chain into it without saying so if the user asked to stop.

## Rules

- Anything outward-facing or hard to reverse (push, deleting, rewriting history) needs an explicit ask.
- Don't re-litigate decisions already recorded in the issue.
- If something blocks you, say what and propose one recommendation; don't enumerate every option.
- When the issue is fully done, mark it in the issue and say what, if anything, remains. Then **ask the
  user whether to close it**; on their agreement run `just close-issue issues/<name>.md` (moves it to
  `issues/done/`, see `issues/issues_rules.md`) and include the move in the final commit. Never move it unasked.
