# Agent harness

Hooks in `.claude/settings.json` (`rust-script` files in `.claude/hooks/`, tested by `test_hooks.rs`):
- `format_rust.rs` (PostToolUse on Edit/Write): runs `cargo fmt --all` after a `.rs` file is edited.
- `clippy_gate.rs` (Stop): if `.rs` files changed, runs `cargo clippy --workspace --all-targets -- -D warnings`;
  on failure the output goes back to the agent, which must fix the code before finishing.

Skills in `.claude/skills/` (structure checked by `.claude/skills/test_skills.rs`):
- `issue-workflow`: how to work an `issues/*.md` file (reformulate, plan, then per deliverable test, code,
  docs, review, human validation, tracking, commit). Architecture decisions go to `docs/adr/`.
- `create-issue`: write a new `issues/*.md` with the template (Goal, Findings, Approach, Resources, Hints), validated by
  `check_issue.rs` (`just check-issue issues/<name>.md`).
- `prioritize-issues`: re-rank `issues/queue.md` (the ordered list of open issues; `just next-issue` prints the first)
  from the roadmap, ADRs, history and each issue, with a validation stop.
- `commit-message`: procedure to write a commit message per `AGENTS.md`, validated by `commit_check.rs`
  (`just check-commit` checks `HEAD`).
- `smoke-test`: end-to-end run on a real repo with the local Ollama and a verdict (`just smoke <repo>`).
- `quality-benchmark`: quality of the generated docs against a hand-written reference on a public repo, user docs hidden then shown, several runs, table with the change since the previous benchmark (`just benchmark benchmark/<repo>/reference.yaml`).
- `update-docs`: checklist to keep the documentation in sync after a change.

Specs (`specs/`, ADR 0032): `.claude/specs/check_specs.rs` (`just check-specs`, part of `just check`) validates the format of
each spec and fails when an `accepted` spec has a `deterministic` requirement no test cites with `// spec: ID`, or a test
cites an unknown id; tested by `.claude/specs/test_check_specs.rs`.
