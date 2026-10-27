---
id: SET
status: accepted
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
- **SET-4** (deterministic) A `retrodoc.toml` that still names a removed key (`[pipeline] body_lines`) must fail to
  load with an error that names the key, says it was removed and tells to delete the line.

## Scenarios

- (SET-1) Given a `retrodoc.toml` written before this change with `rules = false` in it, when it is read, then
  the rules pass stays off.
- (SET-4) Given `body_lines = 5` under `[pipeline]`, when any command loads the config, then it stops with
  `[pipeline] body_lines was removed` and the advice to delete the line, not a bare unknown-field error.
- (SET-3) Given a small library where the joint reading finds fewer entry points, when the user sets
  `business_read = false`, then each pass reads the business files on its own.

## Non-goals

- `[brief] evidence` and `domains_from`: decided by their own issues.
- The quality benchmark does not judge the rules: it always turns the rules pass off (`pass_replay rules` measures them).
- Reading only the start of method bodies: the measure kept the files whole (`issues/settle_experimental_settings.md`).
