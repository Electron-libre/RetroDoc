---
name: prioritize-issues
description: Re-rank the open issues in `issues/queue.md` by reading the roadmap, the ADRs, the recent history and every open issue, then propose the new order for validation. Use when the user asks to prioritize, reorder or re-rank the issues, or after several issues were created or closed and the order may be stale.
---

# Prioritize the issues

Talk to the user in the `user_language` of `.claude/harness.toml` (skills, code, docs and the queue stay in English). The queue is
`issues/queue.md`, one line per open issue, first = next (`just next-issue` reads it). Don't implement
anything here.

Show the user your results as `.claude/rules/reports-to-the-user.md` says.

## Procedure

1. **Current state**: `just next-issue` (also reports files missing from the queue or queue lines without
   a file; fix that drift first), then read `issues/queue.md`.
2. **Context**: the `roadmap` and `architecture` documents of `.claude/harness.toml`, the ADRs in `docs/adr/` that the issues cite, `git log --oneline -30` (what just
   landed and what it unblocks), and `issues/done/` titles when an issue refers to a closed one.
3. **Each open issue**: read its Goal, Findings, Approach and Hints (not only the title). Note for each its
   value (what it unlocks for the product or the pipeline quality), its size, what it depends on, and any
   "priority" or "once X" statement in it. Its size is the changed lines and deliverables its Approach implies,
   checked against the size limit of `skill:issue-workflow` step 1.
4. **Rank** by, in this order: hard dependencies (an issue never before what it needs), value to the product
   vision, then cost (small and cheap first when value is close), then risk and measurability. Keep the
   current order when nothing argues against it: don't reshuffle for the sake of it.
5. **Propose, then STOP**: show the user, in that language, a table `rank | issue | why here` and, below it, only the
   changes against the current queue with the reason for each, then the issues over the size limit with their
   proposed cuts. Ask for validation or corrections.
6. **Apply** on their agreement: split the issues as validated (`skill:issue-workflow` `details.md`, "Split an
   issue"), rewrite the entries of `issues/queue.md` in the validated order (format
   `- [<stem>](<stem>.md) - <why here>`, keep the header text), run `just next-issue` and check it reports
   no drift. When the queue changed, commit `issues/queue.md` alone, with the issue files the split wrote or
   narrowed, added then named on the commit line (`git add <files>`, then
   `git commit -m <msg> -- <files>`; never `git add -A`), with `skill:commit-message`; ask first when the queue held changes that are not this skill's.
