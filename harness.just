# Recipes of the shared agent harness. Managed by `harness sync`: change them in the central repository. The tools they
# run are those of `harness run`, the binary of the revision `.claude/harness.toml` pins.

# For the user only, never the agent: waive the code review of the next commit of this worktree (creates its waiver file and prints the path).
waive-review:
    #!/usr/bin/env sh
    set -e
    waiver=$(git rev-parse --path-format=absolute --git-path claude-review-waived)
    touch "$waiver"
    echo "$waiver"

# Validate a commit message against AGENTS.md (default: HEAD).
check-commit rev="HEAD":
    git log -1 --format=%B {{rev}} | harness run commit-check

# Validate every non-merge commit of base..head (used by CI on pull requests).
check-commits base head:
    harness run check-commits {{base}} {{head}}

# Move a finished issue to issues/done/ and repoint the documents that cite it (only after the user agreed). `--relink` repairs old references.
close-issue issue:
    harness run close-issue {{issue}}

# Which issue to work on next (first entry of issues/queue.md that no branch or worktree has taken) and drift between the queue and the files.
next-issue:
    harness run next-issue

# Give an issue its own worktree `.claude/worktrees/<stem>` and branch `<stem>`, so two agents can work in parallel.
start-issue issue:
    harness run issue-worktree start {{issue}}

# Remove the worktree and the branch of an issue once its branch is merged into the main branch (rebase and one-commit squash included); `--force` for a squash of several commits, only after the user agreed.
finish-issue stem *flags:
    harness run issue-worktree finish {{stem}} {{flags}}

# Validate a new issue file against the create-issue template.
check-issue issue:
    harness run check-issue {{issue}}

# Validate specs/*.md and trace their requirements to the `// spec: ID` comments of the tests.
check-specs *args:
    @harness run check-specs {{args}}

# List the test files that cite no requirement of an accepted spec, largest first: the capabilities worth a spec.
spec-coverage *args:
    @harness run spec-coverage {{args}}

# Fail when a managed harness file was modified or deleted since the last `harness sync`.
check-harness:
    harness check
