---
name: audit-instructions
description: Read the instructions the user gave the agent over time (CLAUDE.md, AGENTS.md, memory, typed messages of the session transcripts of every project) and propose the skills and hooks worth adding, with evidence, for validation. Use when the user asks to audit their instructions, find what to automate, or propose new skills or hooks.
---

# Audit the instructions

Talk to the user in the `user_language` of `.claude/harness.toml` (skills, code, docs and issues stay in English). A recurring multi-step
procedure is a skill, a rule the agent must never break is a hook. Propose only: create nothing here.

## Procedure

1. **Collect**: `rust-script .claude/skills/audit-instructions/audit_sources.rs` lists, per project of
   `~/.claude/projects`, the sessions, the typed messages, the memory files and the instruction files. Then
   read, project by project, `rust-script .claude/skills/audit-instructions/audit_sources.rs --messages`
   (pipe it through `grep -P '^<project>\t'`), the instruction files and the memory files listed. The output
   holds private content: keep it in the conversation, never in a file of the repository.
2. **Detect** candidates:
   - a procedure the user re-explained or re-triggered in several sessions or projects (same steps, same
     wording or intent);
   - a correction given more than once ("no, not like that", "I already told you");
   - a rule phrased "never" / "always" / "jamais" / "toujours", in an instruction file, a memory file or a message.
   Plain validations ("ok", "yes", "continue") are not candidates. Skip what an existing skill in
   `.claude/skills/` or hook in `.claude/hooks/` already covers (say so if it fails to).
3. **Classify** each candidate:
   - **skill**: a procedure with steps, triggered by a request;
   - **hook**: a mechanical check on a tool call (a command, a file path, a diff), deterministic enough to
     block or fix without judgment; declared in `settings.json`;
   - **neither**: a plain rule or preference stays in the instruction file.
4. **Report, then STOP**: a table `candidate | class | evidence | why`. Evidence is the count (messages,
   sessions, projects) and one short quote at most, from the user's own words; never quote a project's
   private content, and follow the `confidential` note of `.claude/harness.toml` for anything that would land in a
   shared file or an issue. Give the report in the conversation; if the user wants a copy, write it outside
   the repository and outside `/tmp` (wiped at reboot), e.g. `~/.local/state/`. Never commit it.
5. **Create** only what the user validates, one issue per proposal through `skill:create-issue`, in generic
   wording (no project name, no raw quote).
