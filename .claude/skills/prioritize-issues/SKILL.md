---
name: prioritize-issues
description: Review the `priority` and `depends_on` of the open issues by reading the roadmap, the ADRs, the recent history and every open issue, then propose the changes for validation. Use when the user asks to prioritize, reorder or re-rank the issues, after several issues were created or closed and the order may be stale, or to convert the `rank` of the issues of an older format.
---

# Prioritize the issues

Each open issue declares in its front matter `priority: high | medium | low`, the issues it needs, `depends_on:
[<stem>, ...]`, and `why: '<the reason for the priority>'` (ADR 0021); `just next-issue` derives the order: the ready
issues by the milestone order of the `roadmap` documents, then priority, then age; then the blocked ones. Don't
implement anything here.

## Procedure

1. **Current state**: `just next-issue` (also reports a missing priority, an unknown dependency, a cycle and an
   unknown milestone). An issue still carrying a `rank: <n>` (a project updated from the format 5) has no priority
   yet: this pass converts it, its `rank` removed.
2. **Context**: the `roadmap` and `architecture` documents of `.claude/cardinal.toml`, the ADRs in `docs/adr/` that the issues cite, `git log --oneline -30` (what just
   landed and what it unblocks), and `issues/done/` titles when an issue refers to a closed one.
3. **Each open issue**: read its Goal, Findings, Approach and Hints (not only the title). Note for each its
   value (what it unlocks for the product or the pipeline quality), its size, the open issues it needs (a "after X"
   or "once X" in its text names a dependency), and any "priority" statement in it. Its size is the changed lines and deliverables its Approach implies,
   checked against the size limit of `skill:issue-workflow` step 1, and its milestone (its `# Milestone` section).
4. **Decide**: `depends_on` holds only the hard dependencies on other issues (an issue never starts before what it
   needs); a condition that is not an issue (a measure, a number of sessions) stays in `why`. The milestone order
   comes first and is the user's: the priority orders only within a milestone. `priority` by value to the product
   vision, then cost (small and cheap first when value is close), then risk and measurability: `high` for what the
   milestone cannot do without, `low` for a comfort, `medium` otherwise. Keep `high` rare: when most open issues
   of a milestone are `high`, lower the ones that are not needed first. Keep what nothing argues against: don't
   reshuffle for the sake of it.
5. **Propose, then STOP**: show the user, in that language, a table `issue | priority | depends_on | why` with only
   the issues that change and the reason for each, then the order `just next-issue` will print, then the issues
   over the size limit with their proposed cuts. You never reorder the milestones (the user's call): when the
   priorities argue for it, say so as a question. Ask for validation or corrections.
6. **Apply** on their agreement: split the issues as validated (`skill:issue-workflow` `details.md`, "Split an
   issue"), rewrite the front matter of each changed issue (`priority`, `depends_on`, `why`; a `rank` removed;
   nothing else of the file), run `just next-issue` and check it prints the proposed order with no problem
   reported. When something changed, commit the re-prioritized issue files alone,
   with the issue files the split wrote or narrowed, added then named on the commit line (`git add <files>`, then
   `git commit -m <msg> -- <files>`), with `skill:commit-message`; ask first when one of those files held changes that are not this skill's.
