---
name: issue-workflow
description: Drive an issue file from `issues/` to done in validated deliverables (reformulate, plan, then per deliverable test, code, docs, review, human validation, issue update, commit). Use when the user hands over an issue file (e.g. `issues/foo.md`) or says to start, continue or resume an issue, so the process doesn't have to be re-explained in each prompt.
---

# Work an issue

The user hands over an issue file in `issues/`. Talk to the user in the `user_language` of `.claude/harness.toml` (skills, code, docs and
commits stay in English). Never skip a validation stop: they are what makes the autonomy safe. The only ones the user can lift are those of
step 4.6 and 4.8, through the Autonomous mode.

Show the user your results as `AGENTS.md` section "Reports to the user" says.

## 0. Resume or start

Read the issue file. If it has a `# Tracking` section, the issue is already planned: show where it stands in two `next` / `left` lines (the next unchecked
deliverable with the recorded mode, the deliverables left by name) and continue at step 4 in the mode recorded there (none recorded: validation at each deliverable), unless the issue
changes a capability whose spec is missing, still `draft` with no validated requirements, or not named in its `# Specs` section:
then do step 2 first. Otherwise start at step 1.

## 1. Reformulate and validate (STOP)

- Read the issue, plus the resources it names (docs, `git log`, the `roadmap` documents of `.claude/harness.toml`, memory).
- Show, short, only what the issue does not already say: `goal`, `out of scope`, `questions` (plus the means when you
  propose a different one than the issue's).
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
- Ask in the same message how to run step 4: **autonomous (default)**, or a validation at each deliverable.
- **Stop and wait for the user to validate the plan.** Then write it in the issue under `# Tracking`
  (checklist `1. [ ] …`, plus a `## Decisions` list for the answers given, starting with the mode).

## 4. For each deliverable, in order

1. **Gather**: read the code and docs it touches; ask about missing information (one batch of questions).
2. **Test first**: write a behavior or end-to-end test that fails for the right reason. When the capability has a
   spec, the test proves a requirement and carries `// spec: ABC-n` (placement in `specs/README.md`); the
   deliverable that traces the last untraced `deterministic` requirement sets the spec `accepted`, and the one
   that makes the last requirement true sets it `implemented`. Follow the test
   conventions of the project (the `test_conventions` note of `.claude/harness.toml` says where they are written). For non-Rust
   deliverables (hooks, skills, scripts) write the script test in Rust (`rust-script`, no bash) and run it with `just test-harness`.
3. **Implement** until the test passes. The clippy gate hook keeps the code warning-free; fix what it reports.
4. **Docs**: run `skill:update-docs` (it covers the specs). If the change is an architectural decision, also write an ADR in
   `docs/adr/` (template and numbering in `docs/adr/README.md`) and update the
   diagrams in the `architecture` document of `.claude/harness.toml` when the flow changed.
5. **Review (mandatory, never skipped)**: run `just check` (fmt, clippy, tests, specs) and
   `just test-harness`, then **invoke `/code-review` on the diff with the Skill tool** and wait for its
   findings; reread the docs you changed. Fix what the review finds, or say why a finding is not fixed.
   Re-run `just check` after the fixes. Step 6 cannot start before the review has run.
   The hook `require_code_review.rs` enforces it: `git commit` is refused (exit 2) while the diff holds a file
   the last review never saw, so write the tracking and the summary of steps 6 and 7 in the issue **before**
   the review (fixes to reviewed files pass; a diff under `issues/` only needs no review). Only the user can
   waive the review, with `just waive-review` (it creates the waiver of the current worktree, also in a linked
   worktree, where the waiver is per worktree); never create that file nor run that recipe yourself. You can
   still name them (commit message, `grep`): the hook only refuses a command that writes the file or runs the recipe.
6. **Human review (STOP, except in autonomous mode)**: show one line `done` (what changed, not the diff), a table `check | result` (tests, lint, specs, docs, code
   review), then the line `Code review:` with the findings and what was done about each (`no findings` if it came back
   clean), then the lists `not verified` (state it plainly) and `decisions needed`, empty when nothing applies. If you did
   not invoke the review, go back to step 5 instead of presenting the deliverable. Wait for the user's validation.
   Don't commit before it. In autonomous mode, write this summary in the issue and go on.
7. **Track**: tick the deliverable in the issue's `# Tracking` and note decisions or follow-ups.
8. **Commit**: after validation (autonomous mode: right after step 7), run `skill:commit-message` (message per `AGENTS.md`, validated by its
   checker). Commit only the files of this deliverable (never `git add -A`; leave unrelated or personal
   files such as `settings.local.json`) and report the hash. If the user wants to commit themselves, output
   just the message.

Then move to the next deliverable (autonomous mode: chain into it). Don't chain into it without saying so if the user asked to stop.

## Autonomous mode

The default once the plan is validated, unless the user asked for a validation at each deliverable. An issue whose
`# Tracking` records no mode (planned before this mode existed) is in per-deliverable validation. The mode is
recorded first in `## Decisions` of the issue's `# Tracking`, so it survives a `/clear` or a resumed thread. The user
can switch it at any time.

- **Lifted**: step 4.6 stops and the wait before step 4.8. Write the 4.6 summary (`done`, `check | result`,
  `Code review:`, `not verified`, `decisions needed`) in the issue instead, then commit locally and chain into the next
  deliverable.
- **Never lifted**: the review of step 4.5 and its checks, the stops of steps 1 to 3, the Rules on push, deletion and
  history rewriting (no push).
- **Stops the chain**: a blocking question, a decision the summary needs from the user, a review finding you can't
  settle, a failing check you can't fix (never commit or build on a red base). After the last deliverable, give the
  final summary: a table `deliverable | commit`, then `left` (what remains, if anything); ask whether to close the issue.

## Parallel agents

Two agents can work two issues at once, each in its own git worktree. From the main worktree run
`just start-issue issues/<name>.md`: it creates `../<repo>-<name>` on a branch `<name>`; open the agent there.
The issue must be committed on the main branch first. While the branch or worktree exists, `just next-issue` skips the issue.
When the issue is done, merge it from the main worktree (rebase the branch onto the main branch, then
fast-forward), run `just close-issue issues/<name>.md` there, commit the move, then `just finish-issue <name>`:
run it from the main worktree (it refuses inside the worktree to remove, `--force` or not): it removes the worktree and the branch only once merged (fast-forward, rebase or squash of any number of commits are detected, the last by comparing trees) and clean.
When the work is not on the main branch, `finish-issue` refuses: after the user agreed, run `just finish-issue <name> --force`, which deletes the branch even though
its work is not on the main branch. `close-issue` refuses in a linked worktree, so
the queue is never edited on an issue branch; in that worktree, report the issue as done and leave the closing
to the merge.

## Rules

- Anything outward-facing or hard to reverse (push, deleting, rewriting history) needs an explicit ask.
- Don't re-litigate decisions already recorded in the issue.
- If something blocks you, say what and propose one recommendation; don't enumerate every option.
- When the issue is fully done, mark it in the issue and say what, if anything, remains. Then **ask the
  user whether to close it** (in a linked worktree, see "Parallel agents": the closing waits for the merge); on their agreement run `just close-issue issues/<name>.md` (moves it to
  `issues/done/`, drops the issue from `issues/queue.md` and rewrites `issues/<name>.md` to
  `issues/done/<name>.md` in every other document that cites it) and include the
  move and those edits in the final commit. Never move it unasked.
- To know which issue comes next, run `just next-issue` (first entry of `issues/queue.md` that no branch or
  worktree has taken) instead of reading the issue files. An issue is taken when a branch named after its stem
  exists or a worktree has it checked out; never start a taken one, `Every issue of the queue is taken.` means
  there is nothing to start. Reordering the queue is the user's call (`skill:prioritize-issues`).
