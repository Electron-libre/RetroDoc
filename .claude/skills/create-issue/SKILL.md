---
name: create-issue
description: Write a new issue file in `issues/` with the repo's template (Goal, Findings, Approach, Resources, Hints) and validate it with a checker. Use when the user asks to create, open or file an issue, or when a smoke test, review or discussion leaves work worth tracking for later.
---

# Create an issue

An issue is a Markdown file `issues/<snake_case_name>.md` that `skill:issue-workflow` can later pick up.
This skill writes it; `check_issue.rs` enforces the mechanical rules. Don't implement anything here.

## Procedure

1. **Check for duplicates**: `ls issues issues/done` and skim the titles close to the subject. If one already
   covers it, propose extending it instead of creating a new file.
2. **Gather the facts** from what was actually observed (logs, smoke runs, code, `git log`). Don't invent
   findings; if a fact is missing, say so in the issue or ask the user.
3. **Write the file** in English (the conversation with the user stays in French), with this template:

   ```markdown
   # <Title: what to achieve, imperative; it names the thread via /rename>

   # Goal
   <The problem and the expected outcome, in a few sentences.>

   # Findings (optional; for a smoke run: date, repo, model)
   * <Measured facts, not guesses.>

   # Approach
   <Measure first when the cause is unknown, then candidate solutions (numbered, can be combined).
   Propose, don't decide: decisions are made in `issue-workflow` step 1-2.>

   # Resources
   * <Files, ADRs, docs, skills to read first.>

   # Hints (optional)
   * <Traps already met, constraints.>
   ```

   - No `# Tracking` section: the plan is written there by `issue-workflow` after the user validated it.
   - Smoke-test repos may be confidential (`CLAUDE.md`): describe them generically, except
     `delivery_router`. No absolute or home paths.
4. **Validate**, and fix the file until it prints nothing:

   ```sh
   just check-issue issues/<name>.md
   ```

5. **Deliver**: give the path and a two-line summary. Don't commit unless asked (then
   `skill:commit-message`), and offer to start `skill:issue-workflow` on it.
