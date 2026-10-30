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
