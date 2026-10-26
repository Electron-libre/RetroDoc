# AGENTS.md

## Commit messages

Follow the Conventional Commits format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Subject line

- 72 characters maximum, including type and scope (shorter is better).
- Format `<type>(<scope>): <description>`: no space before the colon, one space after.
- Lowercase description, no trailing period.
- Imperative present tense, so that it completes the sentence "If applied, this commit will…"
  (e.g. `add`, `fix`, `remove`, not `added`, `fixes`, `removing`).
- Say what changed and why it matters, not how the code was modified.

### Type

Use one of:

- `feat`: new feature
- `fix`: bug fix
- `refactor`: restructuring without behavior change
- `docs`: documentation only
- `test`: add or fix tests
- `perf`: performance improvement
- `build`: build system, dependencies, workspace config
- `ci`: CI configuration
- `chore`: maintenance that fits none of the above

### Scope

- The crate or area touched, from the `commit_scopes` list of `.claude/harness.toml`. Two scopes always exist:
  `docs` (the `docs/` folder, README, CLAUDE.md) and `harness` (`.claude/`, `justfile`, CI).
- Omit the scope (`fix: …`) when the change spans the whole workspace.

### Body (optional)

- Separated from the subject by a blank line.
- Wrapped at 72 characters per line.
- Explains why the change was made and what it affects, not a line-by-line
  recap of the diff. Omit it for trivial changes.

### Footer (optional)

- Separated from the body by a blank line.
- Used for `BREAKING CHANGE: …` and issue references (`Refs: #123`).

### Language and style

- English only.
- Describe only what is in the diff; do not invent context.
- No emojis, markdown headings or code fences.
- Never add `Co-Authored-By` trailers or any AI/tool attribution.
- Output only the commit message, with no preamble.

### Examples

```
feat(harness): report the commit a sync came from

The lock file only held hashes, so a drift could not be traced to a
source revision. Record the revision on the first line.
```

```
fix(docs): repoint the links to the renamed folder
```

## Reports to the user

What the agent shows the user (stops, summaries, final report) is read at a glance:

- Facts go in a table or a checklist, one fact per line; prose only for a decision the user must take.
- No narration of the steps: show the result, not the path.
- Don't repeat what the diff, the issue, a spec or the validated plan already shows.
- Keep what a stop must show; shorten the form, not the content.
