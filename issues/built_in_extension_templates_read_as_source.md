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
