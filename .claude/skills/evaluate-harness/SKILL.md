---
name: evaluate-harness
description: Report whether the recent harness revisions made the agent more efficient, the delivered work better and the rules better followed, with the model, Claude Code version and project mix next to each number, plus the fixed context each revision costs and the skills nobody calls. Use when the user asks to evaluate or measure the harness, compare harness revisions, or see what a skill, hook or rule change brought.
---

# Evaluate the harness

Talk to the user in the `user_language` of `.claude/harness.toml` (skills, code, docs and issues stay in English). Measure only:
change nothing here. The sibling `skill:audit-instructions` proposes rules; this one measures what the rules
brought.

Show the user your results as `AGENTS.md` section "Reports to the user" says.

## Procedure

1. **Collect** from the repository root into `<dir>`, a directory made by `mktemp -d` and removed at the end (the
   rows name projects and issues: never write them in the repository):
   - `harness run session-metrics > <dir>/sessions.tsv`; if the user keeps an archive of the rows
     (`--archive <file>`), pass `--archive` to it and use the archive instead, since it outlives the transcripts; the
     other scripts read only the transcripts still kept, so a group older than them has empty rules, quality and
     fixed context cells: say the data is missing, never read it as a trend;
   - `harness run rule-breaches --summary > <dir>/breaches.tsv`;
   - `harness run delivered-quality <repo> --summary > <dir>/quality-<n>.tsv` for each repository
     the sessions name that has an `issues/done/` directory (the current one at least);
   - `harness run fixed-context > <dir>/context.tsv`;
   - `harness run harness-report --sessions <dir>/sessions.tsv --breaches <dir>/breaches.tsv --quality <dir>/quality-1.tsv … --context <dir>/context.tsv`,
     with `--min-sessions <n>` when the user asks for another sample size than 10.
2. **Read** each table of the report, oldest group first: the tables are the numbers, never recompute them.
   - A trend is a change kept over two groups or more; one group out of line is noise.
   - Before naming a trend, compare the `models`, `versions` and `projects` of the groups it spans: when they
     changed too, say the change may come from them, not from the harness.
   - A `too few` group supports no conclusion; say how many sessions it lacks.
   - A skill `unused` over the last groups is a candidate to drop or to describe better; a hook whose refusals
     grow, or a rule whose `breached_share` stays high, goes to `skill:audit-instructions`, with its counts.
   - Proposing nothing is a valid outcome: with no trend, say so and suggest no change.
3. **Report**: one table per axis (efficiency, quality, rules, skills, fixed context), a row per metric that moved or
   matters, the columns `metric | <the last groups> | trend` (↑, ↓ or →) and a line `confounders` when the mix
   changed. Report the trend, never a pass or a fail, and set no target on agent behaviour: numbers become
   targets once shown. Leave out the rules at 0% and the metrics that did not move unless asked.
4. **Keep**: the report stays in the conversation. When the user wants a copy, write it outside the repository and
   outside `/tmp` (e.g. `~/.local/state/`); anything that lands in a shared file or an issue comes from a report run
   with `--anonymous` and follows the `confidential` note of `.claude/harness.toml`. Never commit it.
