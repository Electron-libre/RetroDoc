---
name: issue-workflow
description: Drive an issue file from `issues/` to done in validated deliverables (understanding, spec and plan written in the issue then validated in one stop, then per deliverable test, code, docs, review, human validation, issue update, commit). Use when the user hands over an issue file (e.g. `issues/foo.md`) or says to start, continue or resume an issue, or hands over the next few issues of the queue to chain, so the process doesn't have to be re-explained in each prompt.
---

# Work an issue

The user hands over an issue file in `issues/`. Never skip a validation stop: they are what makes the autonomy safe.
The only ones the user can lift are those of step 3.6 and 3.8, through the Autonomous mode.
Handed the next few issues of the queue to chain, follow [details.md](details.md), "Drive a queue".

## 0. Resume or start

**Work where the session was started**: the main worktree, or a worktree the user opened on the branch they picked.
Never create, enter or leave a worktree yourself (no `cardinal run worktree open`, no `EnterWorktree`). In a linked worktree,
never redirect to the main one (`git -C`, its paths). **Claim the issue first**: `cardinal run issue-claim
issues/<name>.md`; when it refuses (another session's claim, uncommitted changes), or the worktree lock refuses your
commands (another session works in this worktree), stop and tell the user what it names. In the main worktree, also
stop when a branch `<stem>` exists (an issue worktree of the former `start-issue`). Details: [details.md](details.md).

Read the issue file. A `Queue:` line in its `## Decisions`, `Plan: validated` and a plan of that queue not all ticked: resume the queue ([details.md](details.md), "Drive a
queue"). No `# Tracking` section: start at step 1. If `# Tracking` says `Plan: proposed`, the plan was never
validated: redo the stop of step 2. Otherwise (`Plan: validated`, signed or not, or no marker: planned before it existed) show where it stands in two `next` / `left` lines (the next unchecked
deliverable with the recorded mode, the deliverables left by name) and continue at step 3 in the mode recorded there (none recorded:
validation at each deliverable), unless the issue changes a capability whose spec is missing, still `draft` with no validated
requirements, or not named in its `# Specs` section: then write the spec as step 1 says, keeping `# Tracking` as it is, and validate it (step 2) first. A plan whose
deliverables left, added to what `cardinal run deliverable-size <base commit>` counts, exceed the size limit of step 1
also goes back to the stop of step 2, with the split proposed.

## 1. Prepare in the files (no stop)

The understanding, the spec and the plan are written in the files, not in the console: the files keep the history,
the console only shows the link and the questions.

- Read the issue, plus the resources it names (docs, `git log`, the `roadmap` documents of `.claude/cardinal.toml`, memory).
- **Spec**, when the issue changes what a capability does, in a project that has `specs/README.md` (skip it for docs,
  harness, pure refactor, a measure): **invoke `skill:write-spec` with the Skill tool** before the first `Write` or
  `Edit` under `specs/`, even when the format seems known: the skill finds the spec to amend and checks it. Follow it up to
  its step 4 (`check-specs`), the spec left `draft`; skip its step 5, the validation stop: step 2 replaces it. The tests of step 3 come
  from these requirements, not the other way round.
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
- **Size limit**: estimate the changed lines of each deliverable as `cardinal run deliverable-size` will count them
  (added + removed, outside `issues/` and lock files). At most ~1,000 per deliverable (a `low` review reads it whole),
  ~3,000 and 6 deliverables per issue (the `high` review reads `base..HEAD`, the context grows with each
  deliverable); the tool reads the limits of the project. It counts a move that also edits twice, so move in a
  deliverable of its own (in the issue total, a moved file edited later still counts twice). A deliverable over the limit is cut until each part is under it; an issue over it gets a
  split proposed at step 2 ([details.md](details.md), "Split an issue").

## 2. Validate (STOP)

Show, and nothing else:

- The links to the files written in step 1 (the issue, the spec, the ADR), and the issue summed up in two or three lines;
  one more line when the means differs from the issue's, and the ids of the requirements written.
- The questions you can't answer from the repo, if any, as a table `# | question | options | why | impact`: questions
  numbered `1`, `2`…, options lettered `a`, `b`… with the recommended one marked, so the user answers `1 a, 2 c` (a bare
  `ok` takes every recommendation). Don't ask what has a conventional default. The details stay in the files. Each
  `[NEEDS CLARIFICATION: ...]` marker of the spec (warned of by `cardinal run check-specs`) is one row; the answer replaces
  the marker in the spec.
- One line: the mode of step 3, **autonomous (default)** or a validation at each deliverable.
- When the plan exceeds the size limit: the proposed cuts (issue, deliverables, estimated lines), as a question.

**Wait for the user.** Apply their answers and corrections to the files (the answers go to `## Decisions`). When an
answer changes the plan itself (deliverables, their order) rather than a detail, show the new plan lines and stop
again. Then set `Plan: validated by <name> <<email>> on <YYYY-MM-DD>` (the `user.name` and `user.email` of `git config`, today's
date; either unset: ask the user, never guess one) and the base commit. The signature states who validated, it proves
nothing: the agent reads that identity too. The spec file is the record of the requirements; `## Decisions` doesn't copy them.

## 3. For each deliverable, in order

1. **Gather**: read the code and docs it touches; ask about missing information (one batch of questions).
2. **Test first**: write a behavior or end-to-end test that fails for the right reason. When the capability has a
   spec and the project has `specs/README.md`, the test proves a requirement and carries `// spec: ABC-n` (placement in `specs/README.md`), through the
   binary when that is cheap and stable, else internal with its reason, `// spec: ABC-n (internal: <reason>)`; the
   deliverable that traces the last untraced requirement sets the spec `accepted`. Follow the test
   conventions of the project (the `test_conventions` note of `.claude/cardinal.toml` says where they are written). Write
   the test of a hook, a skill or a script like the others, never in bash.
3. **Implement** until the test passes, then **refactor** under the green tests: remove duplication, rename, simplify,
   no new behaviour. The lint gate hook keeps the code warning-free; fix what it reports.
4. **Docs**: run `skill:update-docs` (it covers the specs). If the change is an architectural decision, also write or finish its ADR in
   `docs/adr/` (template and numbering in `docs/adr/README.md`) and update the
   diagrams in the `architecture` document of `.claude/cardinal.toml` when the flow changed.
5. **Review (mandatory)**: run `cardinal run project-check` (the project's format, lint and tests, and the specs); when `cardinal run deliverable-size`
   exits 1 (over the limit of a deliverable), cut the deliverable before the review; then **invoke `/code-review low <target>` with the Skill tool**, `<target>` the line
   `cardinal run review-target` prints (the diff without `issues/` nor the managed copies; always name the level; it
   refuses an untracked file, which `git diff` leaves out: mark it intent-to-add with the `git add -N` it prints),
   and wait for its findings; reread the docs you changed. Fix the findings on the deliverable's own change, or say
   why one is not fixed; a finding outside the requirements of the deliverable becomes an issue
   (`skill:create-issue`), not a fix in this deliverable.
   Re-run `cardinal run project-check` after the fixes.
   The native `pre-commit` hook (`cardinal run review-gate native`) enforces it: `git commit` fails while the diff holds a file
   the last review never saw, so write the tracking and the summary of steps 6 and 7 in the issue **before**
   the review, all but the signature of step 7, which waits for the validation of step 6 (fixes to reviewed files pass; a diff under `issues/` only, or the close of an issue with the references and the milestone state `close-issue` rewrote, needs no review). Only the user can
   waive the review: never do it yourself ([details.md](details.md), "Review waiver").
6. **Human review (STOP, except in autonomous mode)**: show one line `done` (what changed, not the diff), a table `check | result` (tests, lint, specs, docs, code
   review), one line `red:` opening with the ids its tests trace, then the failure message the test of step 3.2 gave before
   the code (`red: ABC-3, ABC-4: <failure>`, `cardinal run delivered-quality` counts a traced id it lacks; `none` for a
   deliverable with no test), then the line `Code review: <n> findings` (`Code review: no findings` if it came back clean; the
   count first, `cardinal run delivered-quality` reads it) with what was done about each, then the lists `not verified` (state it plainly) and `decisions needed`, empty when nothing applies. Wait for the
   user's validation. Don't commit before it. In autonomous mode, write this summary in the issue and go on.
7. **Track**: tick the deliverable in the issue's `# Tracking`, ending its line with who validated it at step 6 (`(validated by
   <name> <<email>> on <YYYY-MM-DD>)`) or `(autonomous)` in autonomous mode, and note decisions or follow-ups.
8. **Commit**: after validation (autonomous mode: right after step 7), run `skill:commit-message` (message per the project's convention,
   validated by its checker), with the trailer `Validated-by: <name> <<email>>` of step 7 for a deliverable a person
   validated, none for an autonomous one. Commit only the files of this deliverable (leave unrelated or personal files such as
   `settings.local.json`) and report the hash. If the user wants to commit themselves, output
   just the message.

Then move to the next deliverable (autonomous mode: chain into it). Don't chain into it without saying so if the user asked to stop.
After the last deliverable, run `skill:update-docs` on the whole issue (`<base>..HEAD`, the base commit recorded in
`## Decisions`), then invoke `/code-review high <target>` once on it, `<target>` the line `cardinal run review-target <base>` prints; the docs edits and the review fixes are one more
deliverable, steps 3.2 to 3.8 with their own `low` review (the commit gate only knows the files that review saw).

## Autonomous mode

The default once the plan is validated, unless the user asked for a validation at each deliverable. An issue whose
`# Tracking` records no mode (planned before this mode existed) is in per-deliverable validation. The mode is
recorded first in `## Decisions` of the issue's `# Tracking`, so it survives a `/clear` or a resumed thread. The user
can switch it at any time.

- **Lifted**: step 3.6 stops and the wait before step 3.8. Write the 3.6 summary in the issue instead, then commit
  locally and chain into the next deliverable.
- **Never lifted**: the review of step 3.5 and its checks, the `high` review of the whole issue, the stop of step 2
  (in a queue: grouped, never lifted);
  no push, deletion or history rewrite unasked.
- **Stops the chain**: a blocking question, a decision the summary needs from the user, a review finding you can't
  settle, a failing check you can't fix (never commit or build on a red base). After the last deliverable, give the
  final summary: a table `deliverable | commit`, then `left` (what remains, if anything); ask whether to close the issue.

## Advisor

When an advisor tool is available, consult it at these moments:

- the plan written in `# Tracking`, before the stop of step 2 (on a resume or a re-stop, only if the plan changed);
- a test or a check still red after a fix (step 3.3 or 3.5), before trying another approach;
- after the `update-docs` of the whole issue, before its `high` review;
- the stale plan check of a queue that finds an overlap ([details.md](details.md), "Drive a queue"), before going on.

Follow the advice unless a step fails empirically or primary evidence (code, test output, the spec) contradicts it;
then consult once more to reconcile. The advisor never replaces a review or a stop. No minimum number of calls.

## Merge back

On the trunk (the branch of the main worktree), the issue is closed as the Rules below say (`cardinal run close-issue`). On
another branch the merge is the host's: on the user's agreement to close, follow [details.md](details.md), "Merge
back" (close on the branch, push it, open or update its pull request).

## Rules

- After a `cardinal sync` that adds a hook, run `cardinal check` at once and the install it prints: the
  live hooks call the installed `cardinal`, and one it refuses blocks every command, the install included.
- When the issue is fully done, mark it in the issue and say what, if anything, remains. Then **ask the
  user whether to close it** (on a branch other than the trunk, see "Merge back"); on their agreement run `cardinal run close-issue issues/<name>.md` (moves it to
  `issues/done/` and rewrites `issues/<name>.md` to
  `issues/done/<name>.md` in every other document that cites it, relative Markdown links included) and stages the
  move and those edits; commit the close on its own with a bare `git commit` and a message from `skill:commit-message`, never a `git add` of the old path, which
  git refuses (a file it names `staged before the close:` goes in too: unstage it first unless it belongs to the close;
  one named `not staged:` held changes of yours and stays out). Never move it unasked. When it was the last open issue of its milestone,
  `close-issue` sets that milestone `done` itself and prints `milestone <name> done in <document>` (never edit the
  `State:` line yourself: the commit gate lets only the tool's edit through); on that line, propose a release to
  the user (`cardinal run release` with no argument prints the version it would number, without a prompt in auto
  mode) and stop: theirs to validate. On their go, run
  `cardinal run release <version>` (a permission prompt, also in auto mode), let them reread the section, commit it
  after its `low` review (the gate asks for it), then run `cardinal run release <version> --tag`; they push.
  The report of the close ends on a Markdown link to the next issue `close-issue` printed
  (`[<title>](issues/<stem>.md)`), or on its line saying there is none.
- To know which issue comes next, run `cardinal run next-issue` (the first ready issue, its `depends_on` all done, that
  no claim, branch or worktree has taken) instead of reading the issue files. An issue is taken when a session claimed
  it, a branch named after its stem exists or a worktree has it checked out, and stays so while its close waits for a
  merge (`Merge pending:`); never start a taken or a `Blocked:` one, `Every open issue is taken.`
  or `No ready issue` means there is nothing to start. An `On hold:` one (an `on_hold` key, or a milestone with an
  `On hold:` field) waits for the user: handed one, give its reason and start only once they lift the hold or confirm, never while it is also `blocked by`. Reordering the issues is the user's call (`skill:prioritize-issues`).
