---
name: audit-instructions
description: Read the instructions the user gave the agent over time (CLAUDE.md, AGENTS.md, memory, typed messages of the session transcripts of every project) and propose the skills and hooks worth adding, or review the existing instructions for contradictions, duplicates and stale rules, with evidence, for validation. Use when the user asks to audit their instructions, find what to automate, propose new skills or hooks, or clean up the instructions.
---

# Audit the instructions

Propose first: create only what the user validates. What an adopted proposal brought is measured by `skill:evaluate-harness`.

## Procedure

1. **Collect**: `cardinal run audit-sources` lists, per project of the `projects/` of the configuration directory, the sessions, typed
   messages, memory files and instruction files (the memory files a session loads at launch, `CLAUDE.local.md`
   and its imports included, `AGENTS.md`, `.claude/AGENTS.md` and the scoped rules, of each working directory and
   its ancestors up to the repository root), after the user-level instruction files (`CLAUDE.md`, `rules/` and their imports in the configuration
   directory), listed once; read them project by project (`cardinal run audit-sources
   --messages` through `grep -P '^<project>\t'`), keeping the private content in the conversation, never in the
   repository.
   `cardinal run rule-breaches --summary` gives, per rule and revision, the sessions where it applied, where the
   agent broke it and the refusals of a gate (no message text); a rule whose `bytes` is filled is a waste measure
   (large tool results), not an instruction.
2. **Detect** candidates:
   - a procedure the user re-explained or re-triggered in several sessions or projects (same steps, same
     wording or intent);
   - a correction given in several sessions ("no, not like that", "I already told you");
   - a rule phrased "never" / "always" / "jamais" / "toujours", in an instruction or memory file, or in messages;
   - a rule the agent broke in a noticeable share of the sessions where it applied (`breached` over `applied`),
     and still breaks in the latest revisions: a hook candidate, or a rewording when a hook cannot see it; a rule
     only `blocked` is a gate doing its job; a waste rule (its `bytes` filled) is never a candidate: its share stays
     high by design, `skill:evaluate-harness` follows it through `bytes_per_session`.
   Plain validations ("ok", "yes", "continue") are not candidates, nor what one session only shows. Skip what
   an existing skill in `.claude/skills/` or hook in `.claude/hooks/` already covers (say so if it fails to).
3. **Classify** each candidate in the first form that holds it, always-on rule last (a skill pays one description
   line per turn, its body on demand; an instruction file is paid whole on every turn):
   - **skill**: a procedure with steps, triggered by a request;
   - **script** or `cardinal` tool: a computation or a scan the agent would otherwise redo by hand;
   - **hook**: a mechanical step on a tool call (a command, a file path, a diff), needing no judgment: block or
     fix it, a hint after a recurring error, trimming a noisy output, a check right after a write;
   - **rule**: a line in an instruction file; its `why` names what it replaces or trims.
4. **Report, then STOP**: a table `candidate | class | evidence | why`; proposing nothing is a valid outcome.
   Evidence is the counts (messages, sessions, projects) and one short quote at most of the user's own words,
   never a project's private content. The report stays in the conversation; a copy the user asks for goes outside
   the repository and `/tmp` (e.g. `~/.local/state/`), never committed.
5. **Create** only what the user validates, one issue per proposal through `skill:create-issue`, in generic
   wording per the `confidential` note of `.claude/cardinal.toml` (no project name, no raw quote).

## Review the existing instructions

When the user asks to review the instructions themselves rather than to find new ones:

1. **Read whole** what the project ships to the agent: every markdown file of `.claude/skills/`, the `.claude/rules/`,
   `AGENTS.md` and `CLAUDE.md` (a managed copy is read, and fixed, at its source).
2. **List**, with file and line, four families:
   - a contradiction: two rules that ask opposite things, or a text that says not to restate a source and restates it;
   - a duplicate: one rule in several files, or a pointer to a rule with no `paths` (every session loads it already);
   - a stale rule: what Claude Code's own prompt says or a current model does unprompted, a legacy format a tool
     already handles, a detail the tool reports itself;
   - a mechanical rule: a check a hook or a `cardinal run` tool could make.
3. **Check the history** before proposing to drop a rule: the issue of `issues/done/` that added it; a rule kept for
   a past incident stays unless a gate or a test now covers it.
4. **Report, then STOP**: a table `# | family | where | finding`, with the fix proposed per family.
5. **Fix** only what the user accepts, through an issue (`skill:create-issue`, then `skill:issue-workflow`): one
   family per deliverable, so a regression points at its cause, each commit with the `Expected-effect:` of what it
   should move; a mechanical rule becomes its own issue.
