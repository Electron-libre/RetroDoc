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
