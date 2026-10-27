---
id: SET
status: draft
adrs: [0021, 0027, 0028]
---

# Defaults of the pipeline settings

## Intent

The `[pipeline]` section of `retrodoc.toml` turns passes and readings on or off. The user who runs `init` then
`generate` must get the settings the measures settled, and a setting kept as an option must do what its key says.

## Requirements

- **SET-1** (deterministic) A `retrodoc.toml` with no `rules` key in `[pipeline]` must read the rules pass as on;
  `rules = false` must read it as off.
- **SET-2** (deterministic) The `retrodoc.toml` that `init` writes must read back as the default settings, the
  rules pass on.
- **SET-3** (deterministic) A `retrodoc.toml` with no `business_read` key must read the joint reading of the
  business files as on; `business_read = false` must read it as off.

## Scenarios

- (SET-1) Given a `retrodoc.toml` written before this change with `rules = false` in it, when it is read, then
  the rules pass stays off.
- (SET-3) Given a small library where the joint reading finds fewer entry points, when the user sets
  `business_read = false`, then each pass reads the business files on its own.

## Non-goals

- `[brief] evidence` and `domains_from`: decided by their own issues.
- The quality benchmark does not judge the rules: it keeps the rules pass off unless asked.
- `body_lines` until its measure with the joint reading is done (`issues/settle_experimental_settings.md`).
