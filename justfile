# Development commands. `just` lists them; the agent harness uses the same entry points.

default:
    @just --list

# Format, lint (warning-free) and test the whole workspace.
check: fmt clippy test

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

# Test the agent harness itself (hooks and skills). Requires `rust-script`.
test-harness:
    rust-script .claude/hooks/test_hooks.rs
    rust-script .claude/skills/test_skills.rs
    rust-script .claude/skills/commit-message/test_commit_check.rs
    rust-script .claude/skills/smoke-test/test_smoke.rs
    rust-script .claude/skills/issue-workflow/test_close_issue.rs
    rust-script .claude/skills/create-issue/test_check_issue.rs
    rust-script .claude/test_settings.rs

# Smoke test on a real repo with the local LLM (see .claude/skills/smoke-test/SKILL.md). Slow.
smoke *args:
    rust-script .claude/skills/smoke-test/smoke.rs run {{args}}

# Validate a commit message against AGENTS.md (default: HEAD).
check-commit rev="HEAD":
    git log -1 --format=%B {{rev}} | rust-script .claude/skills/commit-message/commit_check.rs

# Move a finished issue to issues/done/ (only after the user agreed).
close-issue issue:
    rust-script .claude/skills/issue-workflow/close_issue.rs {{issue}}

# Validate a new issue file against the create-issue template.
check-issue issue:
    rust-script .claude/skills/create-issue/check_issue.rs {{issue}}
