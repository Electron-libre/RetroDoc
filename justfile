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

# Smoke test on a real repo with the local LLM (see .claude/skills/smoke-test/SKILL.md). Slow.
smoke *args:
    rust-script .claude/skills/smoke-test/smoke.rs run {{args}}

# Validate a commit message against AGENTS.md (default: HEAD).
check-commit rev="HEAD":
    git log -1 --format=%B {{rev}} | rust-script .claude/skills/commit-message/commit_check.rs
