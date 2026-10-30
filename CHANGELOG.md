# Changelog

The notable changes of each version, newest first ([Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
[Semantic Versioning](https://semver.org/)). Each section is written by `just release <version>` from the commits,
then reread before the release is committed.

## [0.2.0] - 2026-10-30

### Milestones

- Language and framework agnostic

### Added

- `scan` and `roles` count the files of the languages `roles.yaml` promotes to source.
- The files no rule of `roles.yaml` matches get a role from their paths, in one more call.

### Fixed

- The roles given to the unmatched files are kept out of `roles.yaml`.
- Templates, markup, stylesheets and data are never promoted to source; only the denied extensions a file holds are
  reported.
- Singular, BDD and Android variant test layouts are read as tests.
- Blade, PHP and Drupal view templates (`.blade.php`, `.html.php`, `.tpl.php`) and CakePHP `.ctp` templates are
  kept out of the source files.

## [0.1.0] - 2026-10-30

### Milestones

- Foundation
- Repo map
- Domains
- Features, use cases and diagrams
- Confidence and debt report
- Writing and CLI ergonomics
- Business-level documentation
- Scalability and cost control
- MCP server over the generated docs
- Top-down redesign

First numbered release.
