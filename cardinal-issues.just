# The recipes of the issue files, imported by `cardinal.just`. Managed by `cardinal sync`: change them in the Cardinal repository.

# Move a finished issue to issues/done/ and repoint the documents that cite it (only after the user agreed). Ends on the next issue to work on. `--relink` repairs old references.
close-issue issue:
    cardinal run close-issue {{issue}}

# Which issue to work on next (the first ready issue no branch or worktree has taken), the blocked ones and the problems of
# the priorities and dependencies.
next-issue *args:
    cardinal run next-issue {{args}}

# Validate a new issue file against the create-issue template.
check-issue issue:
    cardinal run check-issue {{issue}}
