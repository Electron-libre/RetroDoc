# Work an issue: details

The cases `SKILL.md` leaves out, read when one of them comes up.

## Review waiver

Only the user can waive the review of step 3.5, with `just waive-review`: it creates the waiver of the current
worktree (also in a linked worktree, where the waiver is per worktree). Never create that file nor run that recipe
yourself. Deny rules of `.claude/settings.json` refuse the recipe and an edit of the file; naming them (commit
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
branch they pick, with `git worktree add` or `just open-worktree <name>` (creates `../<repo>-<name>` on a branch
`<name>`), and launches a plain `claude` there or opens it as an editor project. Not `claude --worktree`, nor `EnterWorktree` from another
session: Claude Code then isolates the session and refuses every command it cannot prove stays in the worktree.
Several issues may share a worktree, one after the other; the worktree outlives them.

- **Claim**: `issue-claim` writes the ref `refs/cardinal/claims/<stem>` for the session (`CLAUDE_CODE_SESSION_ID`);
  `just next-issue` then skips the issue in every worktree. It takes back a claim of the same session, or of the same
  worktree whose session ended. A new claim needs the worktree clean outside `issues/`.
- **Lock**: the `workspace-lock` hooks keep one live session per worktree; a second one has its edits and commands
  refused. A crashed session's lock is freed when its process is gone.
- **Hooks** act on the work tree of their `cwd` (`lint-gate`, `review-gate`; `format-code` formats the project of the
  edited file); `cardinal run` finds the `cardinal.toml` of the worktree and runs the tools of the revision it pins
  (with a local source, those of the worktree's own crate). The session's project is the worktree: its hooks are
  declared by the worktree's own `.claude/settings.json`, so a hook a branch adds runs at once in that session.

## Merge back

On a branch other than the trunk, once the user agreed to close the issue:

1. `just close-issue issues/<name>.md` on the branch: the claim becomes `merge pending` until the trunk holds the
   close. Commit the move with the last tracking (the milestone set `done` when this was its last open issue, as the
   Rules of `SKILL.md` say; a `low` review when `close-issue` rewrote documents outside `issues/`).
2. Run `just check-cardinal`: when it names a hook the installed `cardinal` refuses (the branch adds a subcommand the
   hooks call), run the `cargo install` it prints before going on. After the merge the hooks of the main worktree
   call it too, and one the installed `cardinal` refuses blocks every command, the install included.
3. Push the branch to the remote the trunk tracks (`git push -u <remote> <branch>`), then, with the host's CLI (`gh`
   on GitHub: `gh pr list --head <branch>`), open the pull request to the trunk when the branch has none, titled after
   the issue, its body the summary of the deliverables; with one, the push updated it. Report its URL. No remote or no
   CLI of the host: stop and print what is left to the user (the push, the pull request, or `git merge --ff-only
   <branch>` in the main worktree).
   Never rebase a pushed branch unasked. The merge is the host's (or the user's).
   No link to the next issue here: the `issues/` of the worktree is the branch's.

`close-issue` refuses an issue of another worktree named by path. `close-worktree` refuses inside the worktree it would remove, and a branch whose work is not
on the main branch unless `--force`.
