# Improve Harness

# Goal

Make the coding agent more autonomous and reduce the repetition of instructions in prompts.

# Approach

Complete the repo's harness with skills and hooks.

# Resources

History of the conversations with the coding agent and commits.
The repo's documentation files.

# Hints

I will give the agent instructions with files like this one (the current document).

For each issue, this is what must be done:

* Reformulate the need and validate the understanding with the user
* Once the understanding is validated, prepare an action plan in several deliverables.
* For each deliverable:
* * Gather the necessary information, ask for the missing information
* * Write a behavior or end-to-end test to validate the deliverable.
* * Write the code that implements the deliverable
* * Update the repo's documentation, architecture decisions, diagrams, etc.
* * Review the code and the documentation.
* * Submit to the user for a human review.
* * Update the issue to track progress.
* * Prepare the commit message.

# Tracking

## Plan (validated)

1. [x] Hooks: `cargo fmt` after an edit, blocking clippy at stop (`.claude/hooks/`, `.claude/settings.json`) — commit 6099318
2. [x] `issue-workflow` skill (issue loop, validation stops, tracking in the issue) + `docs/adr/` — validated
2b. [x] Scripts in `rust-script` (hooks + tests) and `justfile` (`just check`, `just test-harness`) — added at the user's request, validated
3. [x] `commit-message` skill (`AGENTS.md` rules, no attribution) + `commit_check.rs` checker — validated
4. [x] `smoke-test` skill (local Ollama, target repo as a parameter, automatic verdict) — validated
5. [x] Permissions cleanup: shared list in `settings.json`, `settings.local.json` cleaned and git-ignored, `.claude/test_settings.rs` test — awaiting human review

## Decisions

* Skills in English; the agent speaks French with the user.
* The agent may commit, but only after the human review of a deliverable is validated.
* Harness scripts in Rust (`rust-script`), dev commands in a `justfile` (no bash).
* Commit subject: 72 characters maximum (`AGENTS.md` relaxed, in line with actual practice).
* Confidential test repos (client): never named in versioned files; smoke test target passed as a parameter. Default LLM: local Ollama.
* ADRs in `docs/adr/`.
* Clippy failing: the agent fixes it (Stop hook, a single rerun to avoid the loop).
* `.claude/` (except `settings.local.json`) and `issues/` are versioned in the repo.
