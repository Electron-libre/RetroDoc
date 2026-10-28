---
id: SET
status: accepted
adrs: [0019, 0020, 0021, 0027, 0028, 0034, 0035]
---

# Defaults of the pipeline settings

## Intent

The `[pipeline]` and `[brief]` sections of `retrodoc.toml` turn passes and readings on or off. The user who runs `init` then
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
- **SET-5** (deterministic) A `retrodoc.toml` with no `evidence` key in `[brief]` must read the extracts of the
  signals closest to each feature and use case as on; `evidence = false` must read them as off.
- **SET-6** (deterministic) A `retrodoc.toml` with no `domains_from` key in `[pipeline]` must read the domains
  and features as built from the surface (`surface`); `domains_from = "contexts"` must read as that path.
- **SET-7** (deterministic) A `generate` on the `surface` path must warn when its surface holds no entity and no
  entry point while the repository has source files: the run documents no domain and lists its files as
  technical support.
- **SET-8** (deterministic) A `retrodoc.toml` must fail to load when it still names a setting of the removed
  `files` path (`domains_from = "files"` in `[pipeline]`, `batch_chars` in `[llm]`, a `[llm.passes.repo-map]`
  section), with no run of another path, and an error that names the setting, says it was removed (ADR 0035)
  and tells what to change.

## Scenarios

- (SET-1) Given a `retrodoc.toml` written before this change with `rules = false` in it, when it is read, then
  the rules pass stays off.
- (SET-4) Given `body_lines = 5` under `[pipeline]`, when any command loads the config, then it stops with
  `[pipeline] body_lines was removed` and the advice to delete the line, not a bare unknown-field error.
- (SET-3) Given a small library where the joint reading finds fewer entry points, when the user sets
  `business_read = false`, then each pass reads the business files on its own.

- (SET-5) Given a `retrodoc.toml` that `init` wrote before this change, with `evidence = false` in it, when it is
  read, then the extracts stay off: the change only reaches new and hand-written configs.
- (SET-6) Given a `retrodoc.toml` that `init` wrote before the key existed (2026-10-09), with no `domains_from`
  line, when it is read, then `generate` builds the domains from the surface.
- (SET-7) Given a repository in a language the surface passes don't read, when `generate` runs with the default
  config, then the log warns that no domain is documented and that the files are listed as technical support.
- (SET-8) Given a `retrodoc.toml` that `init` wrote between 2026-10-09 and 2026-10-28, with
  `domains_from = "files"` in it, when any command loads the config, then it stops with `domains_from = "files"
  was removed` and the advice to delete the line or set `surface`, not a bare unknown-variant error.

## Non-goals

- `contexts` as the default: it needs the `contexts` map first and documents a slice per run, an opt-in for large applications (ADR 0034).
- Building the domains from the file summaries when the surface is empty: rejected by ADR 0035.
- The quality benchmark does not judge the rules: it always turns the rules pass off (`pass_replay rules` measures them).
- Reading only the start of method bodies: the measure kept the files whole (`issues/done/settle_experimental_settings.md`).
