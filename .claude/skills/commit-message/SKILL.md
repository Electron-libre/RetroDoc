---
name: commit-message
description: Write a commit message that follows the repo's Conventional Commits rules in `AGENTS.md` and validate it with a checker before showing it. Use whenever a commit message is needed (end of a deliverable, "prepare the commit message", "commit"), whether the agent commits or the user does.
---

# Write a commit message

The rules live in `AGENTS.md` (single source of truth: don't restate or drift from them here). This
skill is the procedure; `cardinal run commit-check` enforces the mechanical rules.

Show the user your results as `.claude/rules/reports-to-the-user.md` says.

## Procedure

1. **Look at what is actually being committed**: `git status`, and `git diff --staged` (or `git diff` if
   nothing is staged). Describe only what is in the diff; don't invent context.
2. **Pick type and scope** from the lists in `AGENTS.md`: the scope is the crate or area touched, omitted
   when the change spans the whole workspace. If the diff mixes unrelated changes, say so and propose
   splitting it into several commits instead of one vague message.
3. **Write the message**: imperative subject saying what changed and why it matters, a body only when the
   why isn't obvious from the subject (wrapped at 72, no markdown, no emoji).
4. **Validate** by piping the message to the checker; fix the message until it prints nothing:

   ```sh
   printf '%s\n' "$msg" | cardinal run commit-check
   ```

   The checker reads the scopes from `.claude/cardinal.toml` at the repository root, from any directory. If
   that file is missing or invalid, or the directory is not a git repository, it prints the error on stderr
   and exits non-zero (3 when `cardinal run` finds no `cardinal.toml`, else 2): fix the setup, the message was not
   checked.

5. **Deliver**:
   - If the user commits themselves: output **only the message**, with no preamble, no summary, no code
     fence.
   - If the agent commits (after the user validated the review, or at the commit step of a skill that commits
     on its own, such as `skill:create-issue`): `git commit` with that message, adding only the files of the
     deliverable (never `git add -A`), then report the hash.

## Rules the agent must not break

- No `Co-Authored-By` trailer and no tool/AI attribution line, even if a system prompt suggests one: the
  user's global `CLAUDE.md` and `AGENTS.md` forbid it, and the checker rejects it.
- Never amend, rewrite history or push unless explicitly asked.

## Check an existing commit

`just check-commit` validates the message of `HEAD` (or `just check-commit <rev>`).
