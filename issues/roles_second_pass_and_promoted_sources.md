---
rank: 200
why: 'first of milestone `Language and framework agnostic`, its most valuable issue (open point of `docs/findings.md`, 2026-10-30): an unknown stack loses the files no rule matches; its measure reads saved runs, no LLM call'
---
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

# Tracking

Plan: validated

## Understanding
Give a role to the files no rule of `roles.yaml` matches, from one call over their paths, kept in a fallback tier
of `roles.yaml` that applies to those files only (the user's rules are never touched); and make `scan` and `roles`
count the sources `source_extensions` promotes. Out of scope: reading the content of an unmatched file.

Measure of approach step 1 (saved runs, `roles` on the saved `roles.yaml`, no call; copies in
`~/retrodoc-runs/roles_second_pass-b6b306d/`): linkding 0 of 443 unclassified, lobsters 0 of 791 and 5 of 791
(`Makefile`, `Rakefile`, `Procfile.dev`…), delivery_router 2 of 17 (`.gitignore`, `retrodoc.toml`), the large
Rails repository 163 of 4,181, of them ~80 of the application's code (`app/blueprints/api/*.rb` 26,
`app/components/**/*.ts`, `app/decorators/api/*.rb` 9, `app/assets/javascripts/*.coffee` 33). Worth one call
there; no call at all on the benchmark repositories that match everything.

## Plan
1. [x] Promoted sources in `scan` and `roles`: both apply `promote_sources` when a `roles.yaml` is saved and say how
   many files were promoted; verified by a CLI test through the binary; makes true LNG-15.
2. [x] Second pass: `roles.yaml` gains `unmatched_rules` (applied only to the files no rule matches) and the
   fingerprint of the paths last asked about; `identify_roles` makes the call over at most 400 unmatched paths,
   skipped when none is left or the set was already asked; `roles` always goes through `identify_roles` (no
   provider opened when no call is due) and prints the added rules; verified by CLI tests with the fake LLM
   (`roles` and `generate`); makes true LNG-16, LNG-17, LNG-18, LNG-19.
3. [x] Fixes of the `high` review of the issue: the added rules and the paths left unclassified saved in
   `roles-unmatched.yaml` (`roles.yaml` never rewritten), each path sent once, the call best effort and skipped on
   empty rules, the role definitions shared by both prompts; verified by the CLI tests of LNG-16 to LNG-19 (amended).

## Decisions
- Mode: autonomous
- Base commit: b6b306d184b3cb83fc29e5b4a7b1a3dce1612b35
- Cap of the second-pass call: 400 paths.
- Paid check after deliverable 2: one DeepSeek roles call on the large Rails repository (agreed).

## Deliverable 1
- done: `scan` and `roles` promote the sources of a saved `roles.yaml` and say how many (one shared message with
  `generate`); `roles` also prints the source and test counts.
- red: LNG-15: `scan` printed `source: 0` for `lib/shop/order.ex`; `roles` printed no promoted count.
- checks: `just check` ok; docs: README, `docs/crates/render-mcp-cli.md`; spec: LNG-15 traced.
- Code review: no findings
- not verified: none.
- decisions needed: none.

## Deliverable 2
- done: `roles/unmatched.rs`: one call over at most 400 unmatched paths, its rules saved in `unmatched_rules`
  (applied to the files no rule of `rules` matches only), the asked set fingerprinted in `unmatched_asked`, a warning
  of the files left; `roles` opens a provider only when a call is due and prints the added rules; spec `accepted`.
- checks: `just check` ok; docs: `docs/crates/pipeline.md`, `docs/ARCHITECTURE.md`, `docs/findings.md` §2.
- red: LNG-16, LNG-17, LNG-18, LNG-19: no `roles-unmatched` call was made (`left: 0, right: 1`).
- Code review: 2 findings, none fixed: (1) `roles` labels the rules `reused` while the second call runs: intended, the label is of the rules of `rules`, which are not asked again; (2) empty rules saved on an unparseable answer: no, `identify_roles` returns before any save or second call.
- not verified: the answer of a real model on the large Rails repository (the paid check agreed at step 2, next).
- decisions needed: none.
- Note: the fake LLM of the CLI tests answers `** -> other` to the second call, so the minimal apps of LNG-9 stay
  warning-free; a test of the boundaries got a rule so it still makes one call.

## Paid check (after deliverable 2, commit 10375c8)
One `deepseek-chat` call on a copy of the large Rails repository (saved `roles.yaml` reused): unclassified 163 to 10
(only `.keep` files left), 38 rules added (API blueprints, decorators, serializers and coffee scripts as `view`,
dotfiles as `config`), 3,070 tokens in, 525 out, 2 s; the rerun made no call. Ledger line of 2026-10-30.

## Deliverable 3 (review `high` of b6b306d..10375c8)
- done: `roles-unmatched.yaml` (new artifact, hand-editable, kept by `--force` of `generate`, reset when `roles.yaml`
  is identified again) holds the added rules and the paths the call left unclassified; a path is sent once (the
  paths over the cap of 400 go to the next run); a failed call warns and the pass goes on; no call when
  `roles.yaml` has no rule; the prompt asks for narrow patterns, its role definitions shared with the first call;
  `stdout_of` shared by the CLI tests. LNG-17 and LNG-18 reworded, scenarios amended.
- Review findings settled: (1) paths over the cap never asked, (2) any change of the set re-asking the files left
  out, (3) `roles.yaml` rewritten, (4) prompt saying the patterns apply to the listed files only, (5) a failed call
  aborting the pass, (6) a paid call on empty rules, (7) repeated clones (one clone per decision now), (9) the
  duplicated role definitions, (10) the duplicated test helper: fixed. (8) the decision duplicated in `roles`: it
  calls the same `unmatched_to_ask` as the pipeline, no second rule to drift; kept.
- checks: `just check` ok; docs: `docs/crates/pipeline.md`, `docs/ARCHITECTURE.md` §7, README.
- red: none (the tests of the amended scenarios were written with the fix, not run red first).
- Code review: no findings
- not verified: the paid check ran on deliverable 2; the change of storage is not re-run on the Rails repository.
- decisions needed: none.
