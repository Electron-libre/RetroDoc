---
priority: low
depends_on: [web_ui_application_name]
why: 'the look of the interface, not what it shows: a comfort; after the name issue, which changes the same header'
---
# Give the web interface the colours of the Meta Lambda site and its credit

# Goal

The web interface follows the colour charter of the Meta Lambda site (meta-lambda.com) and says it is made by
Meta Lambda, with its logo, in a credit line that links to the site.

# Findings

* `crates/retrodoc-web/assets/style.css` uses a light palette of its own (`--fg #1f2328`, `--link #0969da`,
  `--bg-side #f6f8fa`, yellow and orange badges for low confidence and stale sections).
* The charter of the site (its `CHARTE.md`, whose source of truth is the `:root` variables of its `style.scss`) is a
  dark theme with one accent, gold: surfaces `--bg #0e0e10`, `--surface #17171b`, `--surface-2 #1f1f25`,
  `--border #2a2a31`; text `--text #e8e6e1`, `--text-muted #9b978e`; gold `--gold #d4af37`, `--gold-light #e8c872`,
  `--gold-dark #8f7328` (never for text); silver `--silver #c0c4c8` only beside gold; red `fail` and green `pass`
  only for failure and success.
* The site has the logo as `static/assets/img/logo.svg` and `logo-mark.svg`.
* The interface serves its assets embedded in the binary (`crates/retrodoc-web/src/assets.rs`): the logo has to be
  copied into `crates/retrodoc-web/assets/`.

# Approach

1. Map the charter onto the stylesheet variables: surfaces, text, gold for the links and the active item of the tree.
2. Decide the badges: low confidence and stale are warnings, not failures; the charter reserves red and green for
   fail and pass, so a warning colour within the charter (gold-dark border, muted text) or the charter's red for
   unbacked sections only.
3. The Mermaid diagrams: its `dark` theme, or theme variables set from the charter, so that they stay readable on
   the dark surface.
4. A footer (or the header) with the logo mark and "Made by Meta Lambda", linking to https://meta-lambda.com.
5. Check the contrast of each text colour on its surface (the charter already forbids gold-dark for text), and
   screenshot the pages with headless Chrome.

# Specs

* `specs/web-ui.md` amended (the credit line; the colours are no requirement).

# Milestone

Web interface for a first-time reader

# Resources

* The charter: `CHARTE.md` and `sass/assets/css/style.scss` of the Meta Lambda site repository (ask the user where).
* `crates/retrodoc-web/assets/style.css`, `crates/retrodoc-web/assets/diagrams.js`, `templates/base.html`.

# Hints

* The charter says gold must stay rare ("if it is everywhere, it signals nothing"): not every link in gold.
* The logo is the user's: ask before committing it into this repository.

# Tracking

Plan: validated

## Understanding
Restyle the interface on the charter of meta-lambda.com (dark surfaces, gold as the one rare accent) and add a credit
line with the logo mark linking to the site. Out of scope: the site's fonts (system fonts stay), a light theme.
Charter: `~/Code/meta-lambda.gitlab.io/CHARTE.md`; logo: its `static/assets/img/logo-mark.svg`.

## Plan
1. [x] Charter and credit: `style.css` mapped onto the charter variables (links in `--text` underlined `--gold-dark`,
   gold for hover, focus and the brand; the tree marks no current item to colour), badges, cited lines and code in charter colours, Mermaid theme
   variables from the charter in `diagrams.js`, `logo-mark.svg` embedded and served as `image/svg+xml`, a footer in
   `base.html`; verified by a page test of the footer and the logo asset, contrast checked against the charter's
   table, screenshots with headless Chrome; makes true WEB-11 (images), WEB-69.

## Decisions
- Mode: autonomous
- Base commit: 5a40af06752732bbb692b2c0be6bcb1c5d0c747f
- Logo: the user agreed to commit `logo-mark.svg` (full-bleed `#17171b`) into `crates/retrodoc-web/assets/`.
- Badges: low confidence and stale are warnings: `--gold-dark` border, `--gold-light` text; red and green unused.
- Mermaid: `base` theme with variables from the charter, not the built-in `dark` (blue and purple).
- Credit: in a footer, not the header.

## Deliverable 1

- done: the stylesheet on the charter (links in `--text` underlined `--gold-dark`, gold for the brand, hover, focus,
  table headers, inline code, cited lines; badges gold; header and code set back to the text colour by deliverable 2), Mermaid `base` theme read from the stylesheet's variables, the
  logo mark served as `image/svg+xml`, a footer credit in `base.html`.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass, `web-ui` accepted again |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | no findings |

- Code review: no findings
- red: WEB-11, WEB-69: `assertion left == right failed: ["/assets/htmx.min.js", "/assets/style.css"]` and `/: no footer in <!doctype html>`
- The escape test of WEB-10 asserted no `<img` at all; narrowed to the injected `<img src=x`.
- Contrast: only charter colours its table allows for text (`--text`, `--text-muted`, `--gold`, `--gold-light`); `--gold-dark` only for borders and underlines.
- Screenshots (headless Chrome, a saved delivery_router run): home, feature, use case with its diagram unfolded, rules, cited file: readable.
- not verified: the other Mermaid diagram kinds than the sequence diagram (none in the run).

## Deliverable 2: fixes of the high review of the issue

- done: the 9 findings of the `high` review: the footer in the viewport of a short page (body as a flex column), the
  Mermaid colours written in `diagrams.js` (a stylesheet cached from an older binary would leave them empty and make
  `mermaid` throw) with the page background behind the labels, inline code and table headers back to the text colour
  (gold kept rare; a link around a code path keeps its hover), the WEB-11 test reading the `src` of every tag and the
  `href` of every `<link>` in either quote, the plan line no longer claiming an open tree item in gold.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md` (the colours of `diagrams.js`) |
| code review | no findings |

- Code review: no findings
- red: none (fixes; the WEB-11 test was rewritten, it stays green on the current page)
- Screenshots again: the 404 page shows the credit without scrolling; a use case diagram with neutral code paths.
