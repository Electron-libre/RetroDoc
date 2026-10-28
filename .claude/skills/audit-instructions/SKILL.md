---
name: audit-instructions
description: Read the instructions the user gave the agent over time (CLAUDE.md, AGENTS.md, memory, typed messages of the session transcripts of every project) and propose the skills and hooks worth adding, with evidence, for validation. Use when the user asks to audit their instructions, find what to automate, or propose new skills or hooks.
---

# Audit the instructions

Talk to the user in the `user_language` of `.claude/harness.toml` (skills, code, docs and issues stay in English).
Propose only: create nothing here. What an adopted proposal brought is measured by `skill:evaluate-harness`.

Show the user your results as `AGENTS.md` section "Reports to the user" says.

## Procedure

1. **Collect**: `harness run audit-sources` lists, per project of `~/.claude/projects`, the sessions, typed
   messages, memory and instruction files; read them project by project (`harness run audit-sources --messages`
   through `grep -P '^<project>\t'`), keeping the private content in the conversation, never in the repository.
   `harness run rule-breaches --summary` gives, per rule and revision, the sessions where it applied, where the
   agent broke it and the refusals of a gate.
2. **Detect** candidates:
   - a procedure the user re-explained or re-triggered in several sessions or projects (same steps, same
     wording or intent);
   - a correction given in several sessions ("no, not like that", "I already told you");
   - a rule phrased "never" / "always" / "jamais" / "toujours", in an instruction file, a memory file or a message;
   - a rule the agent broke in a noticeable share of the sessions where it applied (`breached` over `applied`),
     and still breaks in the latest revisions: a hook candidate, or a rewording when a hook cannot see it; a rule
     only `blocked` is a gate doing its job.
   Plain validations ("ok", "yes", "continue") are not candidates. Skip what an existing skill in
   `.claude/skills/` or hook in `.claude/hooks/` already covers (say so if it fails to).
3. **Classify** each candidate in the first form that holds it, ordered by context cost (a skill body loads on
   demand, an instruction file is paid on every turn), always-on rule last:
   - **skill**: a procedure with steps, triggered by a request;
   - **script** or `harness` tool: a computation or a scan the agent would otherwise redo by hand;
   - **hook**: a mechanical step on a tool call (a command, a file path, a diff), needing no judgment: block or
     fix it, a hint after a recurring error, trimming a noisy output, a check right after a write; declared in
     `settings.json`;
   - **rule**: a line in an instruction file; the row names what it replaces or trims.
4. **Report, then STOP**: a table `candidate | class | evidence | why`. Proposing nothing is a valid outcome:
   a correction seen in one session only is noise, not a candidate. Evidence is the count (messages,
   sessions, projects) and one short quote at most, from the user's own words; never quote a project's
   private content, and follow the `confidential` note of `.claude/harness.toml` for anything that would land in a
   shared file or an issue. Give the report in the conversation; if the user wants a copy, write it outside
   the repository and outside `/tmp`, e.g. `~/.local/state/`. Never commit it.
5. **Create** only what the user validates, one issue per proposal through `skill:create-issue`, in generic
   wording (no project name, no raw quote).
