---
name: create-issue
description: Write a new issue file in `issues/` with the repo's template (Goal, Findings, Approach, Resources, Hints) and validate it with a checker. Use when the user asks to create, open or file an issue, or when a smoke test, review or discussion leaves work worth tracking for later.
---

# Create an issue

An issue is a Markdown file `issues/<snake_case_name>.md` that `skill:issue-workflow` can later pick up.
This skill writes it; `check_issue.rs` enforces the mechanical rules. Don't implement anything here.

Show the user your results as `AGENTS.md` section "Reports to the user" says.

## Procedure

1. **Check for duplicates**: `ls issues issues/done` and skim the titles close to the subject. If one already
   covers it, propose extending it instead of creating a new file.
2. **Gather the facts** from what was actually observed (logs, smoke runs, code, `git log`, the `roadmap` documents of `.claude/harness.toml`). Don't invent
   findings; if a fact is missing, say so in the issue or ask the user.
3. **Write the file** in English (the conversation with the user stays in the `user_language` of `.claude/harness.toml`), with this template:

   ```markdown
   # <Title: what to achieve, imperative>

   # Goal
   <The problem and the expected outcome, in a few sentences.>

   # Findings (optional; for a smoke run: date, repo, model)
   * <Measured facts, not guesses.>

   # Approach
   <Measure first when the cause is unknown, then candidate solutions (numbered, can be combined).
   Propose, don't decide: decisions are made in `issue-workflow` steps 1 to 3.>

   # Specs (optional; when the issue changes what a capability does)
   * <`specs/<capability>.md` and the requirement ids (`ABC-n`) it creates or amends. A spec that does not exist
   yet is named here and written at `issue-workflow` step 2.>

   # Resources
   * <Files, ADRs, docs, skills to read first.>

   # Hints (optional)
   * <Traps already met, constraints.>
   ```

   - No `# Tracking` section: the plan is written there by `issue-workflow` after the user validated it.
   - Confidentiality: follow the `confidential` note of `.claude/harness.toml` (what must not be named in an
     issue; describe it generically). No absolute or home paths.
4. **Validate**, and fix the file until it prints nothing:

   ```sh
   just check-issue issues/<name>.md
   ```

5. **Queue**: a new issue has a priority, so rank it. Read `issues/queue.md` and the issues around the
   candidate places, then insert `- [<stem>](<stem>.md) - <why here>` at the rank that value and dependencies
   justify (not by reflex at the end; the "why" names what it comes after or unblocks). Say the rank and the
   reason to the user, who can move it. If the new issue changes the case for others, say so and suggest
   `skill:prioritize-issues` rather than silently reshuffling. Run `just next-issue` and check it reports
   no drift.
6. **Deliver**: give the path, rank and one line (no other recap). Don't commit unless asked (then
   `skill:commit-message`), and offer to start `skill:issue-workflow` on it.
