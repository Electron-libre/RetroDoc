---
name: update-docs
description: Checklist-driven update of the project documentation (CLAUDE.md, README.md, `docs/`, AGENTS.md, doc comments) after a code change, a new command/flag or config key. Use when finishing a feature, before committing, or when asked to sync/update/audit the docs, so nothing is forgotten.
---

# Update the project documentation

Goal: after a change, every document that describes the touched behavior says the same true thing.
Work through the steps in order; write "n/a" with the reason rather than skipping a step.

Show the user your results as `.claude/rules/reports-to-the-user.md` says.

## 1. Establish what changed

- Given a commit range (a whole issue, `<base>..HEAD`), read `git diff <range>` and `git log <range> --format=%s`;
  otherwise run `git status`, `git diff` (staged + unstaged) and `git log -5 --format=%s`.
- List the changes in one line each, classified with the kinds of change of the map (step 2).
- If the diff is empty, ask which change or commit range to document.

## 2. Map each change to the documents to touch

Read `.claude/update-docs-map.md`: the project's table `change → documents to update`, and the kinds of change
it distinguishes. Any change that matches no row is a gap in the map: update the documents you find by grep
(step 3) and add the row to the map.

These rows hold for every project:

| Change | Update |
|---|---|
| Behaviour of a capability that has a spec in `specs/` added, changed or dropped | `specs/<capability>.md` (requirement, scenario, status), the `// spec: ID` of the tests, `just check-specs` passing; the page of `docs/` or of the `root_docs` that describes the capability to its user; the `architecture` document of `.claude/cardinal.toml` when a component, a boundary or a flow changed |
| Convention (lint, tests, harness) | `CLAUDE.md`, the page that describes the harness |
| Commit scope added or removed (new crate or area) | `commit_scopes` in `.claude/cardinal.toml` |
| A synced file (skill, script, `AGENTS.md`, `cardinal.just`, workflow) must change | not here: fix it in the Cardinal repository, then `cardinal sync` |
| Any change | not `CHANGELOG.md`: `just release` writes its sections from the commits; only reread and reword the section it just wrote |
| Decision that a future reader would undo by accident | a new ADR in `docs/adr/` |

## 3. Detect stale statements

Grep the old and new names of everything touched and read each hit in context:

```sh
rg -n "<name>" CLAUDE.md AGENTS.md docs/ <root_docs> <architecture>
```

`<root_docs>` and `<architecture>` are the files these keys of `.claude/cardinal.toml` name. Also compare against the code: the entry points, the defaults, and the relative links in the docs.

## 4. Edit

- Change only what is now wrong or missing; keep each document's tone and density. Over a commit range, read each
  page it touched whole and rewrite what the successive patches left partial or inconsistent.
- `CLAUDE.md` is the compact guide, `docs/` holds the detail: don't state the same fact in both beyond a summary.
- Don't invent behavior: confirm from the code or the diff.

## 5. Verify

- `just check` if code or doc comments changed.
- Re-run the grep from step 3: no old name should remain unless intentionally historical.

## 6. Report

Finish with a short table: document → updated / n/a (reason) → what changed. Don't commit unless asked;
when asked, follow `AGENTS.md`.
