---
priority: low
why: 'a flaky test makes the project check red now and then; nothing of the product depends on it'
---
# Make the test of the ask page sessions wait for every question it starts

# Goal
`each_browser_session_keeps_its_own_conversation` (`crates/retrodoc-web/tests/ask_page.rs`, WAK-18, WAK-19) fails
now and then, which turns `cardinal run project-check` red for no change of behaviour. It must pass on every run.

# Findings
* 2026-11-25, while working `issues/done/settling_proposes_within_its_first_question.md`: 2 runs of 15 of the test file
  failed at `last_roles(&llm)`: left `[System, User, Assistant, Tool]`, right `[System, User]`.
* Its first `POST /ask` (`question=Hi`, no cookie) starts a question that the test never waits for; its rounds run
  on the shared fake provider while the later questions run, so the last request recorded may be one of its rounds.

# Approach
1. Wait for the question `Hi` to end (`GET /ask/questions/<n>`) before the questions that follow.
2. Or read the roles of the request of the question `By whom?` instead of the last request recorded.

# Resources
* `crates/retrodoc-web/tests/ask_page.rs` (`each_browser_session_keeps_its_own_conversation`, `ask`, `last_roles`).
* `specs/web-ask.md` (WAK-18, WAK-19).
