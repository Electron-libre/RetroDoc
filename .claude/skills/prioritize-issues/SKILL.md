---
name: prioritize-issues
description: Re-rank the open issues (the `rank` of each issue file) by reading the roadmap, the ADRs, the recent history and every open issue, then propose the new order for validation. Use when the user asks to prioritize, reorder or re-rank the issues, or after several issues were created or closed and the order may be stale.
---

# Prioritize the issues

Talk to the user in the `user_language` of `.claude/cardinal.local.toml`, else of `.claude/cardinal.toml` (skills, code, docs and the ranks stay in English). Each open issue
carries its place in its front matter, `rank: <n>` (lowest = next) and `why: '<why here>'` (ADR 0019); `just
next-issue` prints the resulting order. Don't implement
anything here.

Show the user your results as `.claude/rules/reports-to-the-user.md` says.

## Procedure

1. **Current state**: `just next-issue` (also reports the issues without a rank and the shared ranks; an
   `issues/queue.md` left from the format 4 is converted first with `just next-issue --convert`).
2. **Context**: the `roadmap` and `architecture` documents of `.claude/cardinal.toml`, the ADRs in `docs/adr/` that the issues cite, `git log --oneline -30` (what just
   landed and what it unblocks), and `issues/done/` titles when an issue refers to a closed one.
3. **Each open issue**: read its Goal, Findings, Approach and Hints (not only the title). Note for each its
   value (what it unlocks for the product or the pipeline quality), its size, what it depends on, and any
   "priority" or "once X" statement in it. Its size is the changed lines and deliverables its Approach implies,
   checked against the size limit of `skill:issue-workflow` step 1, and its milestone (its `# Milestone` section).
4. **Rank** by, in this order: hard dependencies (an issue never before what it needs), the order of the
   milestones of the `roadmap` documents (an issue of an earlier milestone first; an issue with no milestone keeps
   its current rank unless its value argues for another), value to the product vision, then cost (small and cheap first when value is close), then risk and measurability. Keep the
   current order when nothing argues against it: don't reshuffle for the sake of it.
5. **Propose, then STOP**: show the user, in that language, a table `rank | issue | why here` and, below it, only the
   changes against the current order with the reason for each, then the issues over the size limit with their
   proposed cuts. You never reorder the milestones (the user's call): when the ranking argues for it, say so as a
   question. Ask for validation or corrections.
6. **Apply** on their agreement: split the issues as validated (`skill:issue-workflow` `details.md`, "Split an
   issue"), rewrite the front matter of each open issue in the validated order (`rank` 100,
   200, 300..., `why` updated; nothing else of the file), run `just next-issue` and check it prints that order
   with no issue without a rank nor shared rank. When the order changed, commit the re-ranked issue files alone,
   with the issue files the split wrote or narrowed, added then named on the commit line (`git add <files>`, then
   `git commit -m <msg> -- <files>`; never `git add -A`), with `skill:commit-message`; ask first when one of those files held changes that are not this skill's.
