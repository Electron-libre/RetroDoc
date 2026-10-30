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
`skill:create-issue` (not its queue, commit and deliver steps), citing the first in `# Findings` and the issue it needs in
`# Hints` (`after <stem>`); its queue line goes right after the one it needs. A split at step 2 takes the
deliverables of the other cuts out of the plan, and the new plan is validated again. In a linked worktree, write
no cut: stop and leave the split to the user in the main worktree, where `start-issue` and `next-issue` see it.

## Worktree of an issue

An issue usually runs in the main worktree. For parallel work the user opens one, from the main
worktree: commit the issue with its queue line (`start-issue` refuses an issue not committed on the main branch),
`just start-issue issues/<name>.md` (creates `.claude/worktrees/<name>` on a branch `<name>` and adds
`/.claude/worktrees/` to `.git/info/exclude`), then a plain `claude` launched in that directory. Not `claude
--worktree`, nor `EnterWorktree` from another session: Claude Code then isolates the session and refuses every
command it cannot prove stays in the worktree. While the branch or worktree exists, `just next-issue` skips the issue.
The hooks act on the work tree of their `cwd` (`lint-gate`, `review-gate`; `format-code` formats the project of the
edited file); `cardinal run` finds the `cardinal.toml` of the worktree and runs the tools of the revision it pins (with
a local source, those of the worktree's own crate). The session's project is the worktree: its hooks are declared by
the worktree's own `.claude/settings.json`, so a hook a branch adds runs at once in that session.

## Merge back

In a linked worktree, once the user agreed to close the issue:

1. Commit the last tracking (with the milestone set `done` when this was its last open issue, as the Rules of
   `SKILL.md` say), then `git rebase <main branch>` (the branch checked out by the main worktree); a
   conflict stops here, for the user.
2. Run `just check-cardinal`: when it names a hook the installed `cardinal` refuses (the branch adds a subcommand the
   hooks call), run the `cargo install` it prints before going on. After the fast-forward the hooks of the main worktree
   call it too, and one the installed `cardinal` refuses blocks every command, the install included.
3. Stop and print, without running them, the commands left to the user in the main worktree: `git merge --ff-only
   <name>` (when it refuses because the main branch moved, rebase again in the worktree), `just close-issue
   issues/<name>.md` and the commit of the move (`docs: close ...`, with its `low` review when `close-issue` rewrote
   documents outside `issues/`), then `just finish-issue <name>` (removes the worktree and the branch once merged and
   clean).

`close-issue` refuses in a linked worktree and an issue of one named by path from the main one, so the queue is never
edited on an issue branch. `finish-issue` refuses inside the worktree it would remove, and a branch whose work is not
on the main branch unless `--force`.
