---
name: issue-workflow
description: Drive an issue file from `issues/` to done in validated deliverables (understanding, spec and plan written in the issue then validated in one stop, then per deliverable test, code, docs, review, human validation, issue update, commit). Use when the user hands over an issue file (e.g. `issues/foo.md`) or says to start, continue or resume an issue, so the process doesn't have to be re-explained in each prompt.
---

# Work an issue

The user hands over an issue file in `issues/`. Talk to the user in the `user_language` of `.claude/harness.toml` (skills, code, docs and
commits stay in English). Never skip a validation stop: they are what makes the autonomy safe. The only ones the user can lift are those of
step 3.6 and 3.8, through the Autonomous mode.

Reports to the user (stops, summaries, final report) follow `.claude/rules/reports-to-the-user.md`.

## 0. Resume or start

**Work where the session was started**: the main worktree, or the issue worktree the user opened for parallel work.
Never create, enter or leave a worktree yourself (no `just start-issue`, no `EnterWorktree`). In a linked worktree,
never redirect to the main one (`git -C`, its paths). Stop and ask the user when the issue is taken elsewhere: in the
main worktree, a branch `<stem>` exists; in a linked one, its branch is not `<stem>`. Details: [details.md](details.md).

Read the issue file. No `# Tracking` section: start at step 1. If `# Tracking` says `Plan: proposed`, the plan was never
validated: redo the stop of step 2. Otherwise (`Plan: validated`, or no marker: planned before it existed) show where it stands in two `next` / `left` lines (the next unchecked
deliverable with the recorded mode, the deliverables left by name) and continue at step 3 in the mode recorded there (none recorded:
validation at each deliverable), unless the issue changes a capability whose spec is missing, still `draft` with no validated
requirements, or not named in its `# Specs` section: then write the spec as step 1 says, keeping `# Tracking` as it is, and validate it (step 2) first. A plan whose
deliverables left exceed the size limit of step 1 also goes back to the stop of step 2, with the split proposed.

## 1. Prepare in the files (no stop)

The understanding, the spec and the plan are written in the files, not in the console: the files keep the history,
the console only shows the link and the questions.

- Read the issue, plus the resources it names (docs, `git log`, the `roadmap` documents of `.claude/harness.toml`, memory).
- **Delegate a broad exploration**: a search over many files, the history or the conventions goes to a read-only
  `Explore` subagent (Agent tool), so only its conclusion stays in the main context. Ask it for the paths and lines
  behind each conclusion; read back only the lines a decision of the plan rests on. A targeted read of a known file
  stays in the main thread; so do the spec, the plan, the tests and the commits; the reviews keep going through
  `code-review`.
- **Spec**, when the issue changes what a capability does (skip it for docs, harness, pure refactor, a measure): run
  `skill:write-spec` up to its format check (requirements with stable ids, `deterministic`, left `draft`, a
  `[NEEDS CLARIFICATION: ...]` marker where you would guess); don't do its validation stop: step 2 replaces it. The
  tests of step 3 come from these requirements, not the other way round.
- **Write `# Tracking`** at the end of the issue:

  ```markdown
  # Tracking

  Plan: proposed

  ## Understanding
  <Three lines at most: the goal in your words, what is out of scope, the means if it differs from the issue's.>

  ## Plan
  1. [ ] <deliverable>: <what it does>; verified by <test or check>; makes true <ABC-n>.

  ## Decisions
  - Mode: autonomous
  - Base commit: <`git rev-parse HEAD`, set at validation>
  ```

  Small deliverables, each independently testable and committable, ordered by value and dependency. An issue that
  fits one deliverable has a one-line plan. A decision of architecture also gets its ADR draft in `docs/adr/` now. The spec and ADR drafts are
  committed with the first deliverable.
- **Size limit**: estimate the changed lines of each deliverable (added + removed, outside `issues/` and lock
  files). At most ~1,000 per deliverable (a `low` review reads it whole), ~3,000 and 6 deliverables per issue (the
  `high` review reads `base..HEAD`, the context grows with each deliverable). A pure `git mv` counts as renames; a
  move that also edits counts twice, so move in a deliverable of its own (in the issue total, a moved file edited
  later still counts twice). A deliverable over the limit is cut until each part is under it; an issue over it
  gets a split proposed at step 2 ([details.md](details.md), "Split an issue").

## 2. Validate (STOP)

Show, and nothing else:

- The links to the files written in step 1 (the issue, the spec, the ADR), and the issue summed up in two or three lines;
  one more line when the means differs from the issue's, and the ids of the requirements written.
- The questions you can't answer from the repo, if any, as a table `# | question | options | why | impact`: questions
  numbered `1`, `2`…, options lettered `a`, `b`… with the recommended one marked, so the user answers `1 a, 2 c` (a bare
  `ok` takes every recommendation). Don't ask what has a conventional default. The details stay in the files. Each
  `[NEEDS CLARIFICATION: ...]` marker of the spec (warned of by `just check-specs`) is one row; the answer replaces
  the marker in the spec.
- One line: the mode of step 3, **autonomous (default)** or a validation at each deliverable.
- When the plan exceeds the size limit: the proposed cuts (issue, deliverables, estimated lines), as a question.

**Wait for the user.** Apply their answers and corrections to the files (the answers go to `## Decisions`). When an
answer changes the plan itself (deliverables, their order) rather than a detail, show the new plan lines and stop
again. Then set `Plan: validated` and the base commit. The spec file is the record of the requirements; `## Decisions` doesn't copy them.

## 3. For each deliverable, in order

1. **Gather**: read the code and docs it touches (a broad exploration delegated as in step 1); ask about missing
   information (one batch of questions).
2. **Test first**: write a behavior or end-to-end test that fails for the right reason. When the capability has a
   spec, the test proves a requirement and carries `// spec: ABC-n` (placement in `specs/README.md`), through the
   binary when that is cheap and stable, else internal with its reason, `// spec: ABC-n (internal: <reason>)`; the
   deliverable that traces the last untraced requirement sets the spec `accepted`. Follow the test
   conventions of the project (the `test_conventions` note of `.claude/harness.toml` says where they are written). Write
   the test of a hook, a skill or a script like the others, never in bash.
3. **Implement** until the test passes, then **refactor** under the green tests: remove duplication, rename, simplify,
   no new behaviour. The clippy gate hook keeps the code warning-free; fix what it reports.
4. **Docs**: run `skill:update-docs` (it covers the specs). If the change is an architectural decision, also write or finish its ADR in
   `docs/adr/` (template and numbering in `docs/adr/README.md`) and update the
   diagrams in the `architecture` document of `.claude/harness.toml` when the flow changed.
5. **Review (mandatory, never skipped)**: run `just check` (fmt, clippy, tests, specs); when `git diff --shortstat`
   (new files included) exceeds the size limit of step 1, cut the deliverable before the review; then **invoke `/code-review low` on the diff with the Skill tool** (always name the level)
   and wait for its findings; reread the docs you changed. Fix the findings on the deliverable's own change, or say
   why one is not fixed; a finding outside the requirements of the deliverable becomes an issue
   (`skill:create-issue`), not a fix in this deliverable.
   Re-run `just check` after the fixes. Step 6 cannot start before the review has run.
   The native `pre-commit` hook (`harness run review-gate native`) enforces it: `git commit` fails while the diff holds a file
   the last review never saw, so write the tracking and the summary of steps 6 and 7 in the issue **before**
   the review (fixes to reviewed files pass; a diff under `issues/` only needs no review). Only the user can
   waive the review: never do it yourself ([details.md](details.md), "Review waiver").
6. **Human review (STOP, except in autonomous mode)**: show one line `done` (what changed, not the diff), a table `check | result` (tests, lint, specs, docs, code
   review), one line `red:` opening with the ids its tests trace, then the failure message the test of step 3.2 gave before
   the code (`red: ABC-3, ABC-4: <failure>`, `harness run delivered-quality` counts a traced id it lacks; `none` for a
   deliverable with no test), then the line `Code review: <n> findings` (`Code review: no findings` if it came back clean; the
   count first, `harness run delivered-quality` reads it) with what was done about each, then the lists `not verified` (state it plainly) and `decisions needed`, empty when nothing applies. If you did
   not invoke the review, go back to step 5 instead of presenting the deliverable. Wait for the user's validation.
   Don't commit before it. In autonomous mode, write this summary in the issue and go on.
7. **Track**: tick the deliverable in the issue's `# Tracking` and note decisions or follow-ups.
8. **Commit**: after validation (autonomous mode: right after step 7), run `skill:commit-message` (message per `AGENTS.md`, validated by its
   checker). Commit only the files of this deliverable (never `git add -A`; leave unrelated or personal
   files such as `settings.local.json`) and report the hash. If the user wants to commit themselves, output
   just the message.

Then move to the next deliverable (autonomous mode: chain into it). Don't chain into it without saying so if the user asked to stop.
After the last deliverable, invoke `/code-review high` once on the whole issue (`<base>..HEAD`, the base commit
recorded in `## Decisions`); its fixes are one more deliverable, steps 3.2 to 3.8 with their own `low` review (the
commit gate only knows the files that review saw).

## Autonomous mode

The default once the plan is validated, unless the user asked for a validation at each deliverable. An issue whose
`# Tracking` records no mode (planned before this mode existed) is in per-deliverable validation. The mode is
recorded first in `## Decisions` of the issue's `# Tracking`, so it survives a `/clear` or a resumed thread. The user
can switch it at any time.

- **Lifted**: step 3.6 stops and the wait before step 3.8. Write the 3.6 summary (`done`, `check | result`,
  `red:`, `Code review:`, `not verified`, `decisions needed`) in the issue instead, then commit locally and chain into the next
  deliverable.
- **Never lifted**: the review of step 3.5 and its checks, the `high` review of the whole issue, the stop of step 2, the Rules on push, deletion and
  history rewriting (no push).
- **Stops the chain**: a blocking question, a decision the summary needs from the user, a review finding you can't
  settle, a failing check you can't fix (never commit or build on a red base). After the last deliverable, give the
  final summary: a table `deliverable | commit`, then `left` (what remains, if anything); ask whether to close the issue.

## Merge back

In the main worktree, the issue is closed as the Rules below say (`just close-issue`). In a linked worktree the issue is closed at the merge, which is
the user's: on their agreement to close, follow [details.md](details.md), "Merge back" (rebase, then stop and print
the commands left to the user).

## Rules

- Anything outward-facing or hard to reverse (push, deleting, rewriting history) needs an explicit ask.
- Don't re-litigate decisions already recorded in the issue.
- After a `harness sync` that adds a hook, run `just check-harness` at once and the `cargo install` it prints: the
  live hooks call the installed `harness`, and one it refuses blocks every command, the install included.
- If something blocks you, say what and propose one recommendation; don't enumerate every option.
- When the issue is fully done, mark it in the issue and say what, if anything, remains. Then **ask the
  user whether to close it** (in a linked worktree, see "Merge back"); on their agreement run `just close-issue issues/<name>.md` (moves it to
  `issues/done/`, drops the issue from `issues/queue.md` and rewrites `issues/<name>.md` to
  `issues/done/<name>.md` in every other document that cites it) and include the
  move and those edits in the final commit. Never move it unasked.
- To know which issue comes next, run `just next-issue` (first entry of `issues/queue.md` that no branch or
  worktree has taken) instead of reading the issue files. An issue is taken when a branch named after its stem
  exists or a worktree has it checked out; never start a taken one, `Every issue of the queue is taken.` means
  there is nothing to start. Reordering the queue is the user's call (`skill:prioritize-issues`).
