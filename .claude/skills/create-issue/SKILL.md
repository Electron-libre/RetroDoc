---
name: create-issue
description: Write a new issue file in `issues/` with the repo's template (Goal, Findings, Approach, Resources, Hints) and validate it with a checker. Use when the user asks to create, open or file an issue, or when a smoke test, review or discussion leaves work worth tracking for later.
---

# Create an issue

An issue is a Markdown file `issues/<snake_case_name>.md` that `skill:issue-workflow` can later pick up.
This skill writes it; `cardinal run check-issue` (`just check-issue`) enforces the mechanical rules. Don't implement anything here.

Show the user your results as `.claude/rules/reports-to-the-user.md` says.

## Procedure

1. **Check for duplicates**: `ls issues issues/done` and skim the titles close to the subject. If one already
   covers it, propose extending it instead of creating a new file.
2. **Gather the facts** from what was actually observed (logs, smoke runs, code, `git log`, the `roadmap` documents of `.claude/cardinal.toml`). Don't invent
   findings; if a fact is missing, say so in the issue or ask the user.
3. **Write the file** in English (the conversation with the user stays in the `user_language` of `.claude/cardinal.local.toml`, else of `.claude/cardinal.toml`), with this template:

   ```markdown
   # <Title: what to achieve, imperative>

   # Goal
   <The problem and the expected outcome, in a few sentences.>

   # Findings (optional; for a smoke run: date, repo, model)
   * <Measured facts, not guesses.>

   # Approach
   <Measure first when the cause is unknown, then candidate solutions (numbered, can be combined).
   Propose, don't decide: decisions are made in `issue-workflow` steps 1 and 2.>

   # Specs (optional; when the issue changes what a capability does)
   * <`specs/<capability>.md` and the requirement ids (`ABC-n`) it creates or amends. A spec that does not exist
   yet is named here and written at `issue-workflow` step 1.>

   # Milestone (optional; when the roadmap has milestones)
   <The name of the milestone of the `roadmap` documents this issue serves.>

   # Resources
   * <Files, ADRs, docs, skills to read first.>

   # Hints (optional)
   * <Traps already met, constraints.>
   ```

   - No `# Tracking` section: the plan is written there by `issue-workflow`.
   - Size: estimate the changed lines and deliverables the Approach implies. Over the size limit of
     `skill:issue-workflow` step 1, propose the cuts to the user and, on their agreement, write one issue per cut
     instead (its `details.md`, "Split an issue"); steps 5 to 7 then cover all of them.
   - Confidentiality: follow the `confidential` note of `.claude/cardinal.toml` (what must not be named in an
     issue; describe it generically). No absolute or home paths.
4. **Validate**, and fix the file until it prints nothing:

   ```sh
   just check-issue issues/<name>.md
   ```

5. **Rank**: a new issue has a priority, so rank it. An `issues/queue.md` left from the format 4 is converted
   first (`just next-issue --convert`, in the main worktree). Run `just next-issue` (the open issues, lowest rank first),
   read the issues around the candidate places, then open the file with a front matter

   ```markdown
   ---
   rank: <n>
   why: '<why here>'
   ---
   ```

   at the place that value and dependencies justify (not by reflex at the end; the "why" names what it comes
   after or unblocks): `<n>` halfway between the ranks of its neighbours (`rank` of the files, gaps of 100), never
   editing another issue to make room (the `why` in single quotes, a `'` inside doubled); a tie with a neighbour is reported by `just next-issue` and settled by
   `skill:prioritize-issues`. Say the rank and the
   reason to the user, who can move it. If the new issue changes the case for others, say so and suggest
   `skill:prioritize-issues` rather than silently reshuffling. Run `just next-issue` and check it lists the issue
   at its place and reports no issue without a rank. Then attach it to a milestone of the `roadmap` documents of `.claude/cardinal.toml`: add its link
   (relative to the roadmap file) under that milestone's issues, set a `shaped` milestone `in issues`, name it in
   the `# Milestone` section of the issue and run `just check-issue` again; or tell the user it has none. An `idea`
   or `done` milestone takes an issue only on the user's agreement. Never create or reorder a milestone unasked: the
   milestones, their order and their states are the user's.
6. **Commit**: commit the issue files alone (the new issue files and the roadmap file step 5 edited), added then named on
   the commit line (`git add <files>`, then `git commit -m <msg> -- <files>`; never `git add -A`), with
   `skill:commit-message`; other modified or staged files stay out of the commit. Two exceptions: an issue
   written inside another deliverable (a review finding) is committed with that deliverable, not on its own;
   when the roadmap file already held changes that are not this skill's, ask before committing.
7. **Deliver**: give the path, rank, milestone, commit hash (if step 6 committed) and one line (no other recap), and
   offer to start `skill:issue-workflow` on it.

## Cut a shaped milestone into issues

When the user asks to turn a milestone of the roadmap into issues (its state `shaped`, or `idea` once its goal is
agreed): read its goal and why, then propose the issues that reach it, each sized under the limit of
`skill:issue-workflow` step 1, in the order of their dependencies, as a table `issue | what | estimated lines`.
On the user's agreement, write each with steps 1 to 5 (each names the milestone), then commit them together at
step 6.
