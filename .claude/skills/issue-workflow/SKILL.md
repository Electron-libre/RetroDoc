---
name: issue-workflow
description: Drive an issue file from `issues/` to done in validated deliverables (reformulate, plan, then per deliverable test, code, docs, review, human validation, issue update, commit). Use when the user hands over an issue file (e.g. `issues/foo.md`) or says to start, continue or resume an issue, so the process doesn't have to be re-explained in each prompt.
---

# Work an issue

The user hands over an issue file in `issues/`. Talk to the user in **French** (skills, code, docs and
commits stay in English). Never skip a validation stop: they are what makes the autonomy safe.

## 0. Resume or start

Read the issue file. Name the thread after the issue: its title is the first `# ` heading of the file
(e.g. `# Improve Harness`). Run `/rename <title>` if you can; if you can't (a skill can't type a slash
command), give the user the exact `/rename <title>` line to paste, once, at the start. Do this on resume
too, since a resumed thread may still carry another name. If it has a `# Tracking` section, the issue is already planned: summarize where it
stands (next unchecked deliverable) and continue at step 4, unless the issue changes a capability whose spec is missing, still `draft` with no
validated requirements, or not named in its `# Specs` section: then do step 2 first. Otherwise start at step 1.

## 1. Reformulate and validate (STOP)

- Read the issue, plus the resources it names (docs, `git log`, past conversations, `PLAN.md`, memory).
- Restate the need in your own words: goal, means, constraints, what is out of scope, open questions.
- Ask the questions you can't answer from the repo. Don't ask what has a conventional default.
- **Stop and wait for the user to validate the understanding.**

## 2. Spec (STOP, when the issue changes what a capability does)

- Skip it, and say so, for an issue that changes no behaviour (docs, harness, pure refactor, a measure).
- Otherwise run `skill:write-spec`: read the spec of the capability in `specs/` and write or amend its
  requirements (stable ids, `deterministic` or `quality`), left `draft`. The tests of step 4 come from
  these requirements, not the other way round.
- **Stop and wait for the user to validate the requirements.** The spec file is the record of that decision;
  the plan of step 3 names the requirements, no need to copy them into `## Decisions`.

## 3. Plan the deliverables (STOP)

- Split the work into small deliverables, each independently testable and committable, ordered by value
  and dependency; name the requirements (`ABC-n`) each one makes true. Give your recommended order and why.
- For each: what it does, how it will be verified.
- **Stop and wait for the user to validate the plan.** Then write it in the issue under `# Tracking`
  (checklist `1. [ ] …`, plus a `## Decisions` list for the answers given).

## 4. For each deliverable, in order

1. **Gather**: read the code and docs it touches; ask about missing information (one batch of questions).
2. **Test first**: write a behavior or end-to-end test that fails for the right reason. When the capability has a
   spec, the test proves a requirement and carries `// spec: ABC-n` (placement in `specs/README.md`); the
   deliverable that traces the last untraced `deterministic` requirement sets the spec `accepted`, and the one
   that makes the last requirement true sets it `implemented`. Follow the test
   conventions in `CLAUDE.md` (fake `LlmProvider`, `tempfile::tempdir()`, no network). For non-Rust
   deliverables (hooks, skills, scripts) write the script test in Rust (`rust-script`, no bash) and run it with `just test-harness`.
3. **Implement** until the test passes. The clippy gate hook keeps the code warning-free; fix what it reports.
4. **Docs**: run `skill:update-docs` (it covers the specs). If the change is an architectural decision, also write an ADR in
   `docs/adr/` (template and numbering in `docs/adr/README.md`) and update the
   Mermaid diagrams in `docs/ARCHITECTURE.md` when the flow changed.
5. **Review (mandatory, never skipped)**: run `just check` (fmt, clippy, tests, specs) and
   `just test-harness`, then **invoke `/code-review` on the diff with the Skill tool** and wait for its
   findings; reread the docs you changed. Fix what the review finds, or say why a finding is not fixed.
   Re-run `just check` after the fixes. Step 6 cannot start before the review has run.
6. **Human review (STOP)**: summarize what was done, what was verified and **what was not** (state it
   plainly), decisions needed. The summary has a line `Code review:` with the findings and what was done
   about each (`no findings` if it came back clean); if you did not invoke the review, go back to step 5
   instead of presenting the deliverable. Wait for
   the user's validation. Don't commit before it.
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
  `issues/done/`, drops the issue from `issues/queue.md` and rewrites `issues/<name>.md` to
  `issues/done/<name>.md` in every other document that cites it) and include the
  move and those edits in the final commit. Never move it unasked.
- To know which issue comes next, run `just next-issue` (first entry of `issues/queue.md`) instead of reading
  the issue files. Reordering the queue is the user's call (`skill:prioritize-issues`).
