# Work an issue: details

The cases `SKILL.md` leaves out, read when one of them comes up.

## Review waiver

Only the user can waive the review of step 3.5, with `cardinal run waive-review`: it creates the waiver of the current
worktree (also in a linked worktree, where the waiver is per worktree). Never create that file nor run that tool
yourself. Deny rules of `.claude/settings.json` refuse it, its `just` alias and an edit of the file; naming them (commit
message, `grep`) is fine.

## Split an issue

An issue over the size limit of step 1 is cut where its parts can be delivered and closed alone (a launcher, then
the tools moved to it, then the cleanup), each under the limit. The cuts are proposed to the user before any file
is written. The first cut keeps the original file (at creation, the name of the whole): its Goal, Approach and plan
are narrowed to it, its Findings stay. Each other cut is a new issue written with steps 1 to 4 of
`skill:create-issue` (not its commit and deliver steps), citing the first in `# Findings`, with the priority of the
first and the cut it needs in `depends_on`; an open issue that depended on the first names the cut it needs. A split at step 2 takes the
deliverables of the other cuts out of the plan, and the new plan is validated again. In a linked worktree, write
no cut: stop and leave the split to the user in the main worktree, where `next-issue` sees it.

## Worktree

An issue usually runs in the main worktree, on the trunk. For parallel work the user opens another worktree on a
branch they pick, with `git worktree add` or `cardinal run worktree open <name>` (creates `../<repo>-<name>` on a branch
`<name>`), and launches a plain `claude` there or opens it as an editor project. Not `claude --worktree`, nor `EnterWorktree` from another
session: Claude Code then isolates the session and refuses every command it cannot prove stays in the worktree.
Several issues may share a worktree, one after the other; the worktree outlives them.

- **Claim**: `issue-claim` writes the ref `refs/cardinal/claims/<stem>` for the session (`CLAUDE_CODE_SESSION_ID`);
  `cardinal run next-issue` then skips the issue in every worktree. It takes back a claim of the same session, or of the same
  worktree whose session ended. A new claim needs the worktree clean outside `issues/`.
- **Lock**: the `workspace-lock` hooks keep one live session per worktree; a second one has its edits and commands
  refused. A crashed session's lock is freed when its process is gone, and the lock of a session with no work in
  progress (a clean worktree, no issue claimed) goes to the next session that acts. A project that never runs two
  sessions in one worktree turns the feature `workspace-lock` off; the claim stays.
- **Hooks** act on the work tree of their `cwd` (`lint-gate`, `review-gate`; `format-code` formats the project of the
  edited file); `cardinal run` finds the `cardinal.toml` of the worktree and runs the tools of the revision it pins
  (with a local source, those of the worktree's own crate). The session's project is the worktree: its hooks are
  declared by the worktree's own `.claude/settings.json`, so a hook a branch adds runs at once in that session.

## Merge back

On a branch other than the trunk, once the user agreed to close the issue:

1. `cardinal run close-issue issues/<name>.md` on the branch: the claim becomes `merge pending` until the trunk holds the
   close. Commit what it staged (the move with the last tracking and what it rewrote, the milestone it set `done`
   included) with a bare `git commit` and a message from `skill:commit-message`, as the Rules of `SKILL.md` say; the commit needs no review.
2. Run `cardinal check`: when it names a hook the installed `cardinal` refuses (the branch adds a subcommand the
   hooks call), run the install it prints before going on. After the merge the hooks of the main worktree
   call it too, and one the installed `cardinal` refuses blocks every command, the install included.
3. Push the branch to the remote the trunk tracks (`git push -u <remote> <branch>`), then, with the host's CLI (`gh`
   on GitHub: `gh pr list --head <branch>`), open the pull request to the trunk when the branch has none, titled after
   the issue, its body the summary of the deliverables; with one, the push updated it. Report its URL. No remote or no
   CLI of the host: stop and print what is left to the user (the push, the pull request, or `git merge --ff-only
   <branch>` in the main worktree).
   Never rebase a pushed branch unasked. The merge is the host's (or the user's).
   No link to the next issue here: the `issues/` of the worktree is the branch's.

`close-issue` refuses an issue of another worktree named by path. `cardinal run worktree close` refuses inside the worktree it would remove, and a branch whose work is not
on the main branch unless `--force`.

## Drive a queue

When the user hands over the next few issues ("the next 2 issues", "chain the queue"), the session drives them one
after the other, with no orchestrator subagent (ADR 0026): the reviews, the gates and the claims work as for one issue.
A queue holds 3 issues at most, the number the user gave if smaller.

1. **Take**: on a clean worktree, run `cardinal run next-issue`, claim the issue it offers (`cardinal run issue-claim`), and
   repeat until the queue is full: a claimed issue is skipped, so the next run offers the next ready one, never a
   blocked one. Take every claim before writing any file (a new claim needs a clean worktree, CLM-9).
2. **Prepare**: step 1 for each issue, in order. Each plan adds to its `## Decisions` a line `Rests on:` naming the
   spec ids, ADRs and files it reads or changes, and a line `Queue: <stem>, <stem>...` naming the queue in order.
3. **One grouped stop**: the stop of step 2 for every issue at once, the links and summaries issue by issue, then one
   questions table numbered across the queue, with the mode of each issue. Cross the `Rests on:` of each plan with
   what the plans before it change, and raise each overlap as a question (keep, drop the issue from the queue,
   reorder). Ask also whether to close each issue as soon as it is done (recommended: `delivered-quality` then counts
   the work of each issue apart; else one close request at the end). The stop is grouped, never lifted. Apply the
   answers and sign each plan as step 2 says; a dropped issue keeps `Plan: proposed`, loses its own `Queue:` line and leaves those of the others.
   Then commit the issue files and the spec and ADR drafts of the whole queue in one commit, after its `low` review,
   so no draft of a later issue sits in the diff of an earlier one. That commit is the base of the queue: the last one
   that changed the `Queue:` line (`git log -1 --format=%H -G '^- Queue:' -- issues/<stem>.md`). Free the claim of a dropped issue last: read it (`git cat-file -p
   refs/cardinal/claims/<stem>`) and, only when it names your session, delete it by compare-and-swap (`git
   update-ref -d refs/cardinal/claims/<stem> <value read>`).
4. **Chain**: steps 3.1 to 3.8 of each issue in the mode recorded for it, its `high` review included; the end of an
   issue gives no final summary nor close request (step 7 does), and closes it at once when the user agreed to.
   Before the next issue, check that `cardinal run project-check` is green, the commits of the issue are in `git log` and its
   `# Tracking` is ticked.
5. **Stale plan check**, before each issue after the first: cross the files of `git diff --name-only <base of the
   queue>..HEAD` and the decisions the issues done added with the `Rests on:` of this one, and reread what overlaps.
   No overlap: go. An overlap without impact: note it in `## Decisions` and go. An overlap that changes the plan: stop
   the chain, set the issue back to `Plan: proposed` and do its own step 2 stop; the issues done stay committed. Once
   the user validates it, the chain goes on with it, then the next ones, each with its own check; dropped, it leaves
   the queue as in point 3.
6. **Stops the chain**: the stops of the Autonomous mode, plus a stale plan.
7. **Final report**: a table `issue | commits | state` (`done`, `closed`, `stopped: <why>`, `not started`), then,
   unless they are closed already, one request to close the issues done; on the user's agreement, close each as the
   Rules of `SKILL.md` say, one commit per close.

**Resume**: an issue whose `## Decisions` holds a `Queue:` line belongs to a queue: after a compaction, a `/clear` or a
resumed session, take the first issue of that line whose plan is not all ticked, run the stale plan check if another
issue of the queue is done, and go on with the chain. Every plan of the line ticked: the queue is over, the issue is
resumed alone (step 0 of `SKILL.md`), its close asked as there.
