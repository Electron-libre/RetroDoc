# The recipes of the commit message convention, imported by `cardinal.just`. Managed by `cardinal sync`: change them in the Cardinal repository.

# Validate a commit message against the commit convention (default: HEAD).
check-commit rev="HEAD":
    git log -1 --format=%B {{rev}} | cardinal run commit-check

# Validate every non-merge commit of base..head (used by CI on pull requests).
check-commits base head:
    cardinal run check-commits {{base}} {{head}}
