---
rank: 450
why: 'after `test_layouts_singular_and_variants`, last of milestone `Language and framework agnostic`: follow-up of `template_extensions_never_promoted`, no measured repository holds such templates'
---
# Keep the templates of a built-in source extension out of the source files

# Goal
The walker classifies a file by its last extension (`FileKind::from_path`), so a view template whose last
extension is a built-in source extension is `Source` with no LLM: a Laravel `.blade.php` view, and maybe
`.html.php` or `.tpl.php` templates. They enter the file budget, the actors pass and the business passes as code,
against the non-goal of `specs/language-agnostic.md` ("Reading view templates (.erb, .blade.php, .cshtml) as
source"). Expected: such a template is never a source file, for any stack of the reference set.

# Findings
* 2026-10-30, `issues/done/template_extensions_never_promoted.md` (answer 3 a): the deny list of LNG-20 applies to the
  promotion only; `from_path` makes every `php` file `Source`, `.blade.php` included.
* Not measured: no repository of the benchmark nor of the smoke tests is a Laravel application.

# Approach
1. Measure first: the share of `.blade.php` files on a public Laravel application, and which other compound
   template suffixes of the reference set end in a built-in extension.
2. Candidates: (a) a list of compound template suffixes (`.blade.php`, ...) checked before the extension in
   `from_path`, next to `is_build_script`; (b) the same suffixes added to the deny list of LNG-20 and matched on the
   whole file name.

# Specs
* `specs/language-agnostic.md`: a requirement next to LNG-5 (the walker's built-in source classification).

# Milestone
Language and framework agnostic

# Resources
* `crates/retrodoc-ingest/src/walker.rs` (`FileKind::from_path`, `is_build_script`, `NEVER_SOURCE`).
* `issues/done/template_extensions_never_promoted.md`, ADR 0036.

# Hints
* The Laravel fixture of LNG-9 (`crates/retrodoc-cli/tests/reference_frameworks.rs`) holds no view today: add
  one to prove it.

# Tracking

Plan: validated

## Understanding
A view template whose last extension is built-in (`.blade.php`, `.html.php`, `.tpl.php`) is never source nor test,
neither by `from_path` nor by the promotion (LNG-21). Out of scope: plain `.php` templates told by their directory
(CakePHP 4 `templates/`), generated code (`_templ.go`). Means: candidate (a), the suffixes checked in `from_path`
and in the promotion's filter, next to `is_build_script`.

Measure (2026-10-30, file lists of shallow clones): `.blade.php` is 18 % of the `.php` files of BookStack
(325/1839), 14 % of Pixelfed (368/2722), 0.4 % of Monica (6/1656, Inertia front); `.tpl.php` is 57 of the 437 PHP
files of Drupal 7. No other compound suffix of the reference set ends in a built-in extension (Rails `.html.erb`,
`.json.jbuilder` end in a template extension already denied).

## Plan
1. [x] Compound template suffixes: `from_path` and `promote_to_source` leave `.blade.php`, `.html.php`, `.tpl.php`
   files other; verified by a `scan` test in `stack_tables.rs` and a promotion unit test in `walker.rs`; makes true
   LNG-21.

## Decisions
- Mode: autonomous
- Base commit: 78dc1da8dfce21b0670cca9c66e1383fbae56dce
- 1 a: `ctp` (CakePHP 2/3 templates) added to `NEVER_SOURCE`.

## Deliverable 1
- done: `from_path` and `promote_to_source` leave `.blade.php`, `.html.php`, `.tpl.php` files other (`is_never_code`
  = build script or compound template); `ctp` joins `NEVER_SOURCE`; LNG-21 written, spec back to `accepted`.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass, LNG-21 traced |
| docs | `docs/crates/core-ingest.md`, `specs/language-agnostic.md` |
| code review | `low`, clean |

- red: LNG-21: `scan` gave `(source: 4, test: 1, markdown: 0, other: 1)` for the 4 templates + 1 model; the
  promotion unit test promoted the templates once `php` was named.
- Code review: no findings
- not verified: no run on a real Laravel repository (none in the benchmark or the smoke tests).
- decisions needed: none.
