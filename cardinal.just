# Recipes of Cardinal, the shared agent harness. Managed by `cardinal sync`: change them in the Cardinal repository.
# The tools they run are those of `cardinal run`, the binary of the revision `.claude/cardinal.toml` pins.

# The recipes of each part a feature may leave out live in a file of their own, imported only when present.
import? 'cardinal-commits.just'
import? 'cardinal-issues.just'
import? 'cardinal-specs.just'
import? 'cardinal-review.just'

# Open a worktree `../<repo>-<name>` (or `--path <dir>`) on a new branch `<name>` (or `--branch <branch>`) cut from the main branch, for an agent to work issues in parallel.
[no-cd]
open-worktree name *flags:
    cardinal run worktree open {{name}} {{flags}}

# Remove a worktree (`<name>` for `../<repo>-<name>`, a path, or a branch) and its branch once merged into the main branch (fast-forward, rebase or squash); `--force` also drops work not on the main branch, only after the user agreed.
[no-cd]
close-worktree target *flags:
    cardinal run worktree close {{target}} {{flags}}

# Prepare a release: no argument prints the proposed version; `<version>` writes its CHANGELOG.md section to reread and commit; `<version> --tag` then creates the local tag (never pushed).
release *args:
    cardinal run release {{args}}

# Fail when a managed Cardinal file was modified or deleted since the last `cardinal sync`.
check-cardinal:
    cardinal check
