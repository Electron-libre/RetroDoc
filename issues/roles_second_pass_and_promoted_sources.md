# Classify the files no role rule matches, and show the promoted sources in `scan` and `roles`

# Goal
The roles pass applies the LLM's glob rules mechanically; a file no rule matches stays `unclassified` for good, and
no later pass reads it by role. The second pass over just those paths, planned since the first version of the pass,
was never built. Separately, `generate`, `actors` and `business-files` promote the files whose extension the roles
call names in `source_extensions` from `Other` to `Source`, but `scan` and `roles` still print the walker's built-in
classification, so on a stack the walker does not know they under-report the sources. Expected: the unmatched files
get a role from one small call (or are reported as left), and every command shows the same source count.

# Findings (code read on 2026-10-30; the figures are of the first roles smoke test, `docs/findings.md` §2)
* `crates/retrodoc-pipeline/src/roles/mod.rs:12`: "the planned second pass over just those paths is not implemented
  yet"; `classify` maps a file with no matching rule to `FileRole::Unclassified`.
* On a large Rails repository (4,181 files), about 92% of the files got a role; about 350 stayed unclassified
  (dotfiles, a `Rakefile`, CoffeeScript files): some are business code of a language the rules did not name.
* `promote_sources` is called by `commands/generate/mod.rs:527`, `commands/actors.rs:21`,
  `commands/business_files.rs:22` and the examples, not by `commands/scan.rs` nor `commands/roles.rs`.

# Approach
1. Measure first: the unclassified count and a sample of their paths on the reference repositories of the
   benchmark (saved runs, no LLM call), to know whether the second pass is worth a call.
2. Second pass: one LLM call over the unclassified paths only (paths, no content), answering a role per path or
   per added glob, merged into `roles.yaml` (still editable); bounded in size, skipped when nothing is unmatched,
   part of the roles fingerprint so a rerun calls nothing.
3. `scan` and `roles` apply `promote_sources` when a `roles.yaml` is saved, and say how many files were promoted.

# Specs
* `specs/language-agnostic.md`: amend LNG-2 (the promoted sources are shown by every command) and add a requirement
  for the unmatched files.

# Milestone
Language and framework agnostic

# Resources
* `crates/retrodoc-pipeline/src/roles/mod.rs`, `crates/retrodoc-cli/src/commands/roles.rs`, `commands/scan.rs`.
* ADR 0008 (surface extraction), ADR 0036 (language and framework agnostic).
* `docs/findings.md` §2 step 1.

# Hints
* The rules are cached and hand-editable: the second pass must not overwrite a rule the user edited.
* A rerun with nothing changed must stay a no-op (`specs/rerun.md`).
