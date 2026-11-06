---
priority: low
why: 'a narrative rarely holds a link or a heading; the page stays readable and safe'
---
# Resolve the links and the headings of a narrative rendered as Markdown

# Goal
Since WEB-118 the narrative of a use case is rendered from Markdown on its page, in the generated text of its edit
and in the preview of the edit form, by the function of the collected docs. Two rules of that function fit a
collected doc, not a narrative shown inside another page. Expected: a relative link of a narrative leads where the
Markdown docs would lead, and a heading of a narrative neither breaks the outline of the page nor duplicates an id.

# Findings
* `collected::markdown_html` keeps a relative link as written: on `/domains/<d>/<f>/<u>`, `[guide](docs/guide.md)`
  resolves to `/domains/<d>/<f>/docs/guide.md`, a 404, not to the collected doc at `/docs/docs/guide.md`. The ask page
  renders its answers with the same function and has the same trait.
* It gives each heading an `id` (`anchor`) and keeps its level: `# Title` becomes an `<h1>` inside "What happens".
  The page renders the narrative twice (body and preview of the edit form), so a heading's `id` appears twice.
* Found by the `high` review of `issues/done/web_ui_edit_live_preview.md`; no narrative of a saved run was checked for
  links or headings.

# Approach
1. Measure: count the narratives of the saved runs that hold a link or a heading.
2. Render a narrative with its own options: no heading ids, the headings shifted under the `h2` of the page (or
   shown as strong text).
3. Resolve a relative link against the path of the Markdown page of the use case and map it to `/docs/<path>` when a
   collected doc is there, else show it as text.

# Specs
* `specs/web-ui.md`: amend WEB-118 (headings, relative links).

# Milestone
A web interface that invites to act

# Resources
* `crates/retrodoc-web/src/collected.rs` (`render`, `narrative_html`), `view.rs`, `edit.rs`, `edit_form.rs`, `ask.rs`
* `specs/web-ui.md` WEB-49, WEB-50, WEB-118 to WEB-120

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
A narrative (page, generated text of its edit, preview) renders its headings without ids, two levels down, and
resolves its relative links against its use case's Markdown page to `/docs/<path>`, else as text. Out of scope: the
ask page (keeps the rules of a collected doc), links to other generated pages. The preview moves to
`/preview/domains/<d>/<f>/<u>` so it knows the use case. Measure: 18,223 narratives of `~/retrodoc-runs`, 0 links,
0 headings: the fix is for human edits.

## Plan
1. [x] Headings: a narrative renders with its own options (no heading id, level + 2, capped at `h6`); verified by
   page and preview tests; makes true WEB-122. (autonomous)
2. [x] Links: resolve a relative link of a narrative against the Markdown page of its use case to a collected doc,
   else text; the preview route takes the use case path (404 when unknown); verified by page, edit and preview
   tests; makes true WEB-123, WEB-119, WEB-120 (amended). (autonomous)
3. [x] Fixes of the `high` review: an email autolink kept, `%2E%2E` climbs, the docs dir set with no
   reconciliation; verified by the WEB-123 tests. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 2cc0c0f07d0f8f04951585461eece23cc8172b2a
- Headings: no id, level + 2, capped at `h6` (1 a). Links: resolved to a collected doc, else text; preview route
  takes the use case path (2 a). Ask page out of scope, a non-goal (3 a).

## Deliverable 1 summary
done: a narrative renders its headings two levels down (`h6` at most) with no `id`, on its page, in the generated
text of its edit and in the preview.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (WEB-123 still untraced, spec `draft`) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

red: WEB-122: `<h3>Paying</h3>` missing, the page held `<h1 id="paying">Paying</h1>`

Code review: no findings

not verified: none. decisions needed: none.

## Deliverable 2 summary
done: a relative link or image of a narrative resolves from the Markdown page of its use case to `/docs/<path>`
when a collected doc is there, else shows as its text; the preview is asked at `/preview/domains/<d>/<f>/<u>`
(404 for a use case the run lacks).

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass, `specs/web-ui.md` back to `accepted` |
| docs | `docs/crates/render-mcp-cli.md`, `edit_form.rs` module doc |
| code review | see below |

red: WEB-123: the narrative kept `<a href="../../../../../guide.md#setup">guide</a>` as written

Code review: 1 findings
- the link path was compared without its `%XX` escapes decoded (`my%20guide.md`): fixed, each segment decoded,
  covered by the WEB-123 test.

not verified: the preview in a real browser (the `hx-post` URL is checked by the WEB-119 test only). decisions
needed: none.

## Deliverable 3 summary
done: fixes of the `high` review of the issue.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass, two WEB-123 scenarios added |
| docs | doc comments of `item_doc_links` and `led` |
| code review | see below |

red: WEB-123: `ada@example.org` and `encoded` shown as text; `docs_dir` empty with no reconciliation

`high` review: 10 findings
- email autolink dropped (regression): fixed.
- `docs_dir` empty when the run has no reconciliation: fixed in `item_doc_links`.
- `%2E%2E` read as a name, `%2F` inside a segment: fixed (climbs; a decoded `/` shows as text).
- `safe_url` test-only: fixed, `led` calls it again.
- docs dir absolute or above the repo: not fixed, its pages are outside the repository, whose collected docs are the
  only targets; the links show as text.
- slugs not percent-encoded in the preview URL: not fixed, the same as the existing `/edit` and review actions
  (`Place::href`), slugs are kebab-case.
- a fragment link to a narrative heading goes nowhere since WEB-122: not fixed, the spec keeps fragment links as
  written (the page's own ids).
- double lookup in `preview`, linear scan of the collected docs, `preview` URL filled for a domain or a feature form:
  not fixed, simplifications with no wrong output.

Code review: no findings

not verified: none. decisions needed: none.
