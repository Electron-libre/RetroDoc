# 0014. Agent harness: hooks and skills, scripted with `rust-script` and `just`

Status: Accepted

_Retroactive ADR, reconstructed from the history (cc05771, 6099318, fbbe7ce, 80256dd, 5a2ec32, 0f8aa0f, 07d59db, 36f5331; 2026-10-05)._

## Context

The coding agent had to be told the same things in every prompt (format, keep clippy green, how to work
an issue, how to write a commit message, how to smoke test), and it forgot some of them. Dev tooling
was also split between bash hooks and ad hoc commands.

## Decision

Version the harness in the repository, shared with everyone:

- Hooks (`.claude/hooks/`): `cargo fmt --all` after a `.rs` edit; a Stop gate that runs
  `cargo clippy --workspace --all-targets -- -D warnings` and sends failures back to the agent.
- Skills (`.claude/skills/`): `issue-workflow` (reformulate, plan, then per deliverable test, code,
  docs, review, human validation, tracking, commit; ADRs under `docs/adr/`), `commit-message` (checked
  by `commit_check.rs`, subject limit 72 characters), `smoke-test` (end-to-end on a real repo with the
  local Ollama, never naming confidential repos), `update-docs`.
- Scripts are `rust-script` files and `just` recipes (`check`, `test-harness`, `smoke`,
  `check-commit`, close-issue), not bash, with tests for the hooks and skill structure. A settings
  hygiene test keeps a shared allowlist without opening arbitrary execution; personal settings are
  git-ignored. Finished issues move to `issues/done/`, after asking the user.

Bash scripts were rejected: one language for the product and its tooling, and testable.

## Consequences

Agent behaviour is reproducible and reviewable; tests protect the harness itself. Contributors need
`rust-script` and `just`. Skills are prose and drift unless `update-docs` is applied.
