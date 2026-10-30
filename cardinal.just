# Recipes of Cardinal, the shared agent harness. Managed by `cardinal sync`: change them in the Cardinal repository.
# The tools they run are those of `cardinal run`, the binary of the revision `.claude/cardinal.toml` pins.

# For the user only, never the agent: waive the code review of the next commit of this worktree (creates its waiver file and prints the path).
waive-review:
    #!/usr/bin/env sh
    set -e
    waiver=$(git rev-parse --path-format=absolute --git-path claude-review-waived)
    touch "$waiver"
    echo "$waiver"

# Validate a commit message against AGENTS.md (default: HEAD).
check-commit rev="HEAD":
    git log -1 --format=%B {{rev}} | cardinal run commit-check

# Validate every non-merge commit of base..head (used by CI on pull requests).
check-commits base head:
    cardinal run check-commits {{base}} {{head}}

# Move a finished issue to issues/done/ and repoint the documents that cite it (only after the user agreed). Ends on the next issue to work on. `--relink` repairs old references.
close-issue issue:
    cardinal run close-issue {{issue}}

# Which issue to work on next (the open issue of lowest rank that no branch or worktree has taken), and the issues without a rank; `--convert` turns a format 4 issues/queue.md into ranks.
next-issue *args:
    cardinal run next-issue {{args}}

# Give an issue its own worktree `.claude/worktrees/<stem>` and branch `<stem>`, so two agents can work in parallel.
start-issue issue:
    cardinal run issue-worktree start {{issue}}

# Remove the worktree and the branch of an issue once its branch is merged into the main branch (rebase and one-commit squash included); `--force` for a squash of several commits, only after the user agreed.
finish-issue stem *flags:
    cardinal run issue-worktree finish {{stem}} {{flags}}

# Prepare a release: no argument prints the proposed version; `<version>` writes its CHANGELOG.md section to reread and commit; `<version> --tag` then creates the local tag (never pushed).
release *args:
    cardinal run release {{args}}

# Validate a new issue file against the create-issue template.
check-issue issue:
    cardinal run check-issue {{issue}}

# Validate specs/*.md and trace their requirements to the `// spec: ID` comments of the tests.
check-specs *args:
    @cardinal run check-specs {{args}}

# List the test files that cite no requirement of an accepted spec, largest first: the capabilities worth a spec.
spec-coverage *args:
    @cardinal run spec-coverage {{args}}

# Fail when a managed Cardinal file was modified or deleted since the last `cardinal sync`.
check-cardinal:
    cardinal check
