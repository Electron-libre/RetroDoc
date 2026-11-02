# The recipe of the review gate, imported by `cardinal.just`. Managed by `cardinal sync`: change it in the Cardinal repository.

# For the user only, never the agent: waive the code review of the next commit of this worktree (creates its waiver file and prints the path).
waive-review:
    #!/usr/bin/env sh
    set -e
    waiver=$(git rev-parse --path-format=absolute --git-path claude-review-waived)
    touch "$waiver"
    echo "$waiver"
