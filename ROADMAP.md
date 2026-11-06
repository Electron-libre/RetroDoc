# Roadmap

The planned work of the project, grouped into milestones, in the order they are meant to land: the user orders them.
A milestone becomes issues once it is shaped (`skill:create-issue`); the `priority` and `depends_on` of each issue file order them within a milestone (`cardinal run next-issue`).
States: `idea`, `shaped`, `in issues`, `done`; `cardinal run release` adds `- Released: v<version>` to a `done` one
and moves it to [ROADMAP-done.md](ROADMAP-done.md). A milestone carries no date unless the project gives it one.

The released milestones are in [ROADMAP-done.md](ROADMAP-done.md); the ones up to "A large application, one bounded
context at a time" are the phases 1 to 11 of the former `PLAN.md`, in their order. The dated findings behind them are
in `docs/findings.md` and the closed issues in `issues/done/`.

## A web interface that invites to act

- Goal: the forms of the web interface (ask, review, edit, filters of the debt and the rules, run comparison,
  search) and its common elements (buttons, fields, tables, badges, navigation) follow the current conventions of
  a documentation tool: visible states (hover, focus, disabled, pending, saved, error), immediate feedback, the next
  action shown on each page, the same components everywhere; checked by screenshots of each page in headless
  Chrome before and after, and a checklist of the states of each form.
- Why: the forms are bare browser controls laid out in columns (`style.css` styles them in a few lines), the actions
  a page offers are not shown, and the look does not match what a user expects of a tool of 2026.
- Constraints: ADR 0039 stays (server-rendered `askama` + `htmx`, assets vendored, no Node toolchain); the colour
  charter (dark surfaces, gold as the one accent) is kept. The interactions are done on the server in Rust with
  `htmx` (live search, Markdown preview with `pulldown-cmark`, validation, notices, loading indicators, keyboard
  shortcuts); a small vendored script only where `htmx` cannot, no WebAssembly.
- State: in issues
- Issues:
  - [web_ui_components](issues/done/web_ui_components.md)
  - [web_ui_form_feedback](issues/done/web_ui_form_feedback.md)
  - [web_ui_filters_and_compare](issues/done/web_ui_filters_and_compare.md)
  - [web_ui_live_search](issues/done/web_ui_live_search.md)
  - [web_ui_next_action](issues/done/web_ui_next_action.md)
  - [web_ui_ask_composer](issues/done/web_ui_ask_composer.md)
  - [web_ui_edit_live_preview](issues/web_ui_edit_live_preview.md)
  - [web_ui_narrative_links_and_headings](issues/web_ui_narrative_links_and_headings.md)

## A local model, measured

- Goal: one complete benchmark run of an application on the local model, with its figures and duration next to the
  hosted baseline, or the principle of `PRODUCT.md` that a small local model is a valid target amended.
- Why: the only complete local attempt on linkding (2026-10-27) failed on the 120 s timeout; the loopback default is
  900 s since, and no local run has been made again.
- State: in issues
- Issues:
  - [complete_local_benchmark_run](issues/complete_local_benchmark_run.md)

## Read by someone taking an application over

- Goal: the generated docs of an application read by a person who takes it over (developer, product owner), with
  what they found right, wrong, missing and useless, written as dated findings.
- Why: the users of `PRODUCT.md` are assumed: only the author has read generated docs so far, and the benchmark
  measures likeness to a reference, not use.
- State: idea
- On hold: deferred by the user (2026-11-04).
