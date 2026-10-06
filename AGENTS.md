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

- The crate or area touched, without the `retrodoc-` prefix:
  `core`, `ingest`, `llm`, `pipeline`, `render`, `mcp`, `cli`, or `docs`.
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
feat(pipeline): add progress reporting to long passes

Long LLM passes gave no feedback, so a slow call looked like a hang.
Log one line per unit with rank, percentage and ETA.
```

```
fix(llm): retry on 429 with exponential backoff
```
