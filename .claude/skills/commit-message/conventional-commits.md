# Commit messages

The convention of the feature `conventional-commits` of Cardinal; a project that turns the feature off follows its
own, stated in its `CLAUDE.md`.

Follow the Conventional Commits format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

## Subject line

- 72 characters maximum, including type and scope (shorter is better).
- Format `<type>(<scope>): <description>`: no space before the colon, one space after.
- Lowercase description, no trailing period.
- Imperative present tense, so that it completes the sentence "If applied, this commit will…"
  (e.g. `add`, `fix`, `remove`, not `added`, `fixes`, `removing`).
- Say what changed and why it matters, not how the code was modified.

## Type

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

## Scope

- The crate or area touched, from the `commit_scopes` list of `.claude/cardinal.toml`. Two scopes always exist:
  `docs` (the `docs/` folder, README, CLAUDE.md) and `cardinal` (`.claude/`, `justfile`, CI).
- Omit the scope (`fix: …`) when the change spans the whole workspace.

## Body (optional)

- Separated from the subject by a blank line.
- Wrapped at 72 characters per line.
- Explains why the change was made and what it affects, not a line-by-line
  recap of the diff. Omit it for trivial changes.

## Footer (optional)

- Separated from the body by a blank line.
- Used for `BREAKING CHANGE: …`, issue references (`Refs: #123`) and the person who validated the change
  (`Validated-by: <name> <<email>>`, set by `skill:issue-workflow`).

## Language and style

- English only.
- No emojis, markdown headings or code fences.

## Examples

```
feat(cardinal): report the commit a sync came from

The lock file only held hashes, so a drift could not be traced to a
source revision. Record the revision on the first line.
```

```
fix(docs): repoint the links to the renamed folder
```
