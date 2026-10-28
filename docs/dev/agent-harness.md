# Agent harness

Most of it is shared with the other projects and copied here by `harness sync` from the central repository
(skills other than `smoke-test` and `quality-benchmark`, which stay RetroDoc's own and whose scripts are tested by `just test-skill-scripts`, `harness.just`, `AGENTS.md`,
`specs/README.md`, `.github/workflows/harness.yml`, `.github/install_harness.sh`). The hooks and checkers are no longer
scripts copied into the project: they are tools of the `harness` binary, run as `harness run <tool>` (`harness run` lists
them) with the binary of the revision `.claude/harness.toml` pins. Never edit those copies: `just check-harness`
fails when one drifted, and the fix belongs in the central repository. What is specific to RetroDoc is in
`.claude/harness.toml` (commit scopes, root documents, roadmap, confidentiality note) and
`.claude/update-docs-map.md` (which documents to update after a change).

The `harness` binary (needed by `just check-harness`, `just check-specs`, hence `just check`, and by the hooks) is installed from
the central repository (`cargo install --git <source in .claude/harness.toml> harness --locked`); `harness update` moves the
project to the head of the central repository and syncs, `harness sync` refreshes the copies.

The attribution is off in `.claude/settings.json` (`"attribution": { "commit": "", "pr": "" }`), so no commit or PR credits Claude.

Hooks in `.claude/settings.json` (`harness run <tool>`):
- `format-rust` (PostToolUse on Edit/Write): runs `cargo fmt --all` after a `.rs` file is edited.
- `clippy-gate` (Stop): if `.rs` files changed, runs `cargo clippy --workspace --all-targets -- -D warnings`;
  on failure the output goes back to the agent, which must fix the code before finishing.
- `review-gate` (review gate): refuses a `git commit` until the `code-review` skill has run on the current diff
  (`record` on PostToolUse `Skill`, `check` on PreToolUse `Bash`, `native` from the git `pre-commit` hook installed by `harness sync`).
  The user waives the review of the next commit by creating `claude-review-waived` in the git directory.

Skills in `.claude/skills/`:
- `issue-workflow`: how to work an `issues/*.md` file (reformulate, plan, then per deliverable test, code,
  docs, review, human validation, tracking, commit). Architecture decisions go to `docs/adr/`. `just start-issue` / `just finish-issue`
  give an issue its own git worktree and branch (`harness run issue-worktree`).
- `create-issue`: write a new `issues/*.md` with the template (Goal, Findings, Approach, Resources, Hints), validated by
  `just check-issue issues/<name>.md`.
- `prioritize-issues`: re-rank `issues/queue.md` (the ordered list of open issues; `just next-issue` prints the first)
  from the roadmap, ADRs, history and each issue, with a validation stop.
- `commit-message`: procedure to write a commit message per `AGENTS.md`, validated by `just check-commit`
  (checks `HEAD`).
- `smoke-test`: end-to-end run on a real repo with the local Ollama and a verdict (`just smoke <repo>`); reads and appends the ledger of measures (`~/retrodoc-runs/LEDGER.md`).
- `quality-benchmark`: quality of the generated docs against a hand-written reference on a public repo, user docs hidden then shown, several runs, table with the change since the previous benchmark (`just benchmark benchmark/<repo>/reference.yaml`); reads and appends the same ledger.
- `write-spec`: write or amend a `specs/*.md` (requirements with stable ids, scenarios, non-goals) with the template,
  validated by `just check-specs`; the user validates the spec before the tests are written.
- `update-docs`: checklist to keep the documentation in sync after a change.
- `audit-instructions`: read the instructions given to the agent over time and propose skills and hooks worth adding, for validation.
- `evaluate-harness`: report whether recent harness revisions made the agent more efficient, the work better and the rules
  better followed, plus the fixed context each revision costs and the skills nobody calls.

Specs (`specs/`, ADR 0032): `harness run check-specs` (`just check-specs`, part of `just check`) validates the format of
each spec and fails when an `accepted` spec has a `deterministic` requirement no test cites with `// spec: ID`, or a test
cites an unknown id; it warns when a requirement is proven only by tests under `src` with no `// spec: ID (internal: <reason>)`.
`just spec-coverage` lists the test files that cite no requirement of an accepted spec, largest first.
