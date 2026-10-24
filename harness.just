# Recipes of the shared agent harness. Managed by `harness sync`: change them in the central repository.

# Test the agent harness itself (hooks, skills, spec checker). Requires `rust-script`.
test-harness:
    for t in .claude/hooks/test_*.rs .claude/skills/test_skills.rs .claude/skills/*/test_*.rs .claude/specs/test_*.rs .claude/test_settings.rs; do [ -e "$t" ] || continue; rust-script "$t" || exit 1; done

# Validate a commit message against AGENTS.md (default: HEAD).
check-commit rev="HEAD":
    git log -1 --format=%B {{rev}} | rust-script .claude/skills/commit-message/commit_check.rs

# Validate every non-merge commit of base..head (used by CI on pull requests).
check-commits base head:
    rust-script script/check_commits.rs {{base}} {{head}}

# Move a finished issue to issues/done/ and repoint the documents that cite it (only after the user agreed). `--relink` repairs old references.
close-issue issue:
    rust-script .claude/skills/issue-workflow/close_issue.rs {{issue}}

# Which issue to work on next (first entry of issues/queue.md) and drift between the queue and the files.
next-issue:
    rust-script .claude/skills/issue-workflow/next_issue.rs

# Validate a new issue file against the create-issue template.
check-issue issue:
    rust-script .claude/skills/create-issue/check_issue.rs {{issue}}

# Validate specs/*.md and trace their requirements to the `// spec: ID` comments of the tests.
check-specs:
    rust-script .claude/specs/check_specs.rs

# Fail when a managed harness file was modified or deleted since the last `harness sync`.
check-harness:
    harness check
