# Work an issue: details

The rare cases of `SKILL.md`, read when one of them comes up.

## Review waiver

Only the user can waive the review of step 3.5, with `just waive-review`: it creates the waiver of the current
worktree (also in a linked worktree, where the waiver is per worktree). Never create that file nor run that recipe
yourself. Deny rules of `.claude/settings.json` refuse the recipe and an edit of the file; naming them (commit
message, `grep`) is fine.

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
