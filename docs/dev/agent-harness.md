# Agent harness

Most of it is shared with the other projects and copied here by `harness sync` from the central repository
(hooks, skills other than `smoke-test` and `quality-benchmark`, which stay RetroDoc's own and are still tested by `just test-harness`, spec checker, `harness.just`, `AGENTS.md`,
`specs/README.md`, `script/`, `.github/workflows/harness.yml`). Never edit those copies: `just check-harness`
fails when one drifted, and the fix belongs in the central repository. What is specific to RetroDoc is in
`.claude/harness.toml` (commit scopes, root documents, roadmap, confidentiality note) and
`.claude/update-docs-map.md` (which documents to update after a change).

The `harness` binary (needed by `just check-harness`, hence `just check`) is installed from a checkout of the central repository
(`cargo install --path crates/harness`); `harness sync` refreshes the copies.

The attribution is off in `.claude/settings.json` (`"attribution": { "commit": "", "pr": "" }`), so no commit or PR credits Claude.

Hooks in `.claude/settings.json` (`rust-script` files in `.claude/hooks/`, tested by `test_hooks.rs`):
- `format_rust.rs` (PostToolUse on Edit/Write): runs `cargo fmt --all` after a `.rs` file is edited.
- `clippy_gate.rs` (Stop): if `.rs` files changed, runs `cargo clippy --workspace --all-targets -- -D warnings`;
  on failure the output goes back to the agent, which must fix the code before finishing.
- `require_code_review.rs` (review gate): refuses a `git commit` until the `code-review` skill has run on the current diff
  (`record` on PostToolUse `Skill`, `check` on PreToolUse `Bash`, `native` from the git `pre-commit` hook installed by `harness sync`).
  The user waives the review of the next commit by creating `claude-review-waived` in the git directory.

Skills in `.claude/skills/` (structure checked by `.claude/skills/test_skills.rs`):
- `issue-workflow`: how to work an `issues/*.md` file (reformulate, plan, then per deliverable test, code,
  docs, review, human validation, tracking, commit). Architecture decisions go to `docs/adr/`. `just start-issue` / `just finish-issue`
  give an issue its own git worktree and branch (`issue_worktree.rs`).
- `create-issue`: write a new `issues/*.md` with the template (Goal, Findings, Approach, Resources, Hints), validated by
  `check_issue.rs` (`just check-issue issues/<name>.md`).
- `prioritize-issues`: re-rank `issues/queue.md` (the ordered list of open issues; `just next-issue` prints the first)
  from the roadmap, ADRs, history and each issue, with a validation stop.
- `commit-message`: procedure to write a commit message per `AGENTS.md`, validated by `commit_check.rs`
  (`just check-commit` checks `HEAD`).
- `smoke-test`: end-to-end run on a real repo with the local Ollama and a verdict (`just smoke <repo>`).
- `quality-benchmark`: quality of the generated docs against a hand-written reference on a public repo, user docs hidden then shown, several runs, table with the change since the previous benchmark (`just benchmark benchmark/<repo>/reference.yaml`).
- `write-spec`: write or amend a `specs/*.md` (requirements with stable ids, scenarios, non-goals) with the template,
  validated by `just check-specs`; the user validates the spec before the tests are written.
- `update-docs`: checklist to keep the documentation in sync after a change.
- `audit-instructions`: read the instructions given to the agent over time and propose skills and hooks worth adding, for validation.

Specs (`specs/`, ADR 0032): `.claude/specs/check_specs.rs` (`just check-specs`, part of `just check`) validates the format of
each spec and fails when an `accepted` spec has a `deterministic` requirement no test cites with `// spec: ID`, or a test
cites an unknown id; tested by `.claude/specs/test_check_specs.rs`.
