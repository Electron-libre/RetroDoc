---
rank: 350
why: 'before `test_layouts_singular_and_variants`, same milestone: already seen on a real repository (templates and stylesheets counted as source), while no run has shown the test layouts yet'
---
# Keep templates, markup and stylesheets out of the promoted sources

# Goal
The roles pass asks the LLM for the `source_extensions` of the repository, and `promote_to_source` turns every
`Other` file with one of them into `Source` (or `Test`). Only the prompt keeps templates, markup, data and assets
out; no code does. When the LLM lists a template or stylesheet extension, those files enter the file budget, the
actors pass and the business passes as code. Expected: an extension of templates, markup, stylesheets or data is
never promoted, whatever the LLM answers, for any stack.

# Findings
* 2026-10-30, `high` review of `issues/done/view_template_labels.md`: `promote_to_source`
  (`crates/retrodoc-ingest/src/walker.rs`) filters build scripts and sbt `project/` files only; the roles prompt
  says "not templates, markup, data or assets" (`crates/retrodoc-pipeline/src/roles/mod.rs`).
* 2026-10-30, large Rails repository, deepseek-chat, saved `roles.yaml` of the second roles pass: `source_extensions`
  = `rb, js, ts, erb, jbuilder, scss`. The `.erb` templates (1,253 files) and the stylesheets are therefore
  promoted to `Source` on that repository, against `docs/findings.md` §5.
* On linkding (`py, js, ts, mjs, astro`), Lobsters and delivery_router the answers held no template extension.

# Approach
1. Measure first: on the large Rails repository, the promoted files per extension and what they displace in the
   file budget and the actors pass.
2. Candidates: (a) a generic deny list of markup, template, stylesheet and data extensions applied after the LLM
   answer (`erb`, `haml`, `slim`, `html`, `jinja`, `hbs`, `mustache`, `twig`, `css`, `scss`, `sass`, `less`, `json`,
   `yaml`...), a stack accelerator in the sense of ADR 0036; (b) a sniff of the content (a share of markup lines)
   before promoting; (c) a stricter prompt with the excluded extensions dropped and reported in the log.
   Open question: `jbuilder` and the like are code that renders data (Ruby), keep or drop.

# Specs
* `specs/language-agnostic.md`: amends the promotion requirements (LNG-2, LNG-15).

# Milestone
Language and framework agnostic

# Resources
* `crates/retrodoc-ingest/src/walker.rs` (`promote_to_source`), `crates/retrodoc-pipeline/src/roles/mod.rs`
  (prompt, `RoleRules::promote_sources`).
* `issues/done/roles_second_pass_and_promoted_sources.md`, `issues/done/view_template_labels.md`.
* ADR 0036 (stack knowledge as accelerators only).

# Hints
* `vue`, `svelte` and `astro` files hold code and markup together; linkding lists `astro`: decide before
  denying them.

# Tracking

Plan: validated

## Understanding
Approach step 1 measured without LLM on the saved copy of the large Rails repository (run of the ledger, 2026-10-30): the LLM names
`rb, js, ts, erb, jbuilder, scss, css, rake`, so 1,253 `erb` + 53 `jbuilder` + 52 `scss` + 28 `css` = 1,386 files
become `Source` next to 2,280 `rb`, i.e. ~38 % of the source set of the file ranking, business files and chunk check.
Means: candidate (a), a deny list in `promote_to_source`, the dropped extensions logged (LNG-20). Out of scope: the
built-in `php` making `.blade.php` source, the prompt text.

## Plan
1. [x] Deny list of template, markup, stylesheet and data extensions in `promote_to_source`, returned to the callers
   and logged by the roles pass; verified by a walker test and a `scan` test through the binary; makes true LNG-20.
2. [x] Fixes of the `high` review of the issue; verified by the walker test over the whole list and `scan`/`roles`
   tests through the binary; makes true LNG-20.

## Decisions
- Mode: autonomous
- Answer 1 a: `jbuilder` and `rabl` are denied (view templates of Rails).
- Answer 2 a: `vue`, `svelte`, `astro` stay promotable (code and markup together).
- Answer 3 a: `.blade.php` and the other templates the built-in extensions make source go to a new issue.
- Base commit: e8d3b82f93c7f7fe8df4d72d364cd15eb13db17c

## Deliverable 1
- done: `promote_to_source` leaves out the extensions of `NEVER_SOURCE` (templates, markup, stylesheets, data;
  `vue`, `svelte`, `astro` kept); `scan`, `roles` and `generate` print the ones `roles.yaml` named; LNG-20 added,
  spec back to `accepted`; findings §5, `docs/crates/pipeline.md`, `docs/crates/render-mcp-cli.md` updated.
- red: LNG-20: `scan` printed `source: 5` for `source: 2` (erb, jbuilder, scss promoted); the walker test did not
  compile (`denied_source_extensions` missing).
- Code review: no findings.
- not verified: no run on the large Rails repository (the measure is the file count of the saved copy).

## Deliverable 2 (`high` review of `e8d3b82..HEAD`, 8 findings)
- done: `promote_to_source` returns a `Promotion` (count + denied extensions some file holds), one API instead of two;
  the message names only the extensions a file holds; templates added (`phtml`, `latte`, `mako`, `ftl`, `ftlh`, `vm`,
  `gsp`, `aspx`, `ascx`, `master`, `vbhtml`, `jst`, `eta`, `xsl`, `xslt`); the walker test iterates `NEVER_SOURCE`;
  a `roles` test; the run path of the confidential repository removed from this issue.
- red: LNG-20: not observed apart, the tests were written with the fix (the old code printed a `json` no file holds).
- Code review: 8 findings (high); low review of the fixes: no findings. Fixed: missing templates, message for absent files, `roles` untested, test subset, two
  APIs, confidential path (still in commit 969d32e, local, not pushed). Not fixed: data extensions
  can't be overridden (the Goal of the issue denies data on purpose, validated plan); `.blade.php` through
  `from_path` (issue `built_in_extension_templates_read_as_source`).
- not verified: `generate` printing the line through the binary (it calls the same `roles::promote_sources`).
