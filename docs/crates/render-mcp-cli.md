# Crates: render, mcp, web and cli

Detail moved out of `CLAUDE.md`. Overview: `docs/ARCHITECTURE.md` §3, §6 and §9.

- **retrodoc-render**: writes the Markdown/Mermaid output to the docs dir (`output.docs_dir`). Modules: `markdown/`
  (the entry points, paths and links in `mod.rs`; one file per page: `agent_index.rs`, `domain.rs`, `feature.rs`,
  `use_case.rs`; the tests by topic in `tests/`), `rules.rs`, `doc_links.rs`, `plan.rs`. `render()` builds
  the files in memory (deterministic), `plan()` compares them with the disk (created/updated + unified diff,
  stale files under `functional/` reported but never deleted), `WritePlan::apply()` writes only what differs.
  `_retrodoc/run-metadata.json` has a timestamp that is ignored when it's the only change, so reruns are no-ops. `functional/llms.txt` (`agent_index`) is the `llms.txt`-style entry point for agents that don't
  run the MCP server: domains and features with links and confidence, how to read them and a pointer to `retrodoc mcp`.
  The CLI drops generated docs (`functional/`, `_retrodoc/`) from the ingested existing docs.
  The rules the web interface shares with the pages are public: `TECHNICAL_HEADING` / `TECHNICAL_NOTE`, `anchor`
  (a heading's anchor), `kind_label` (a rule's kind), `source_ref_label` (cited lines, `path:start-end`), `DocLinks::of`.
  `render_with_docs` (`doc_links.rs`, `specs/docs-reconciliation.md`) adds, from the reconciliation of the debt report (`DocLinks`, built by the CLI's `doc_links`), an "Existing documentation" section to the page of each domain, feature and use case that a doc section documents or contradicts (relative link from the page to the doc file, anchor of the heading, `⚠️ states otherwise: …` for a contradiction), and an `Existing doc:` line under a rule on the feature and use case pages (not for a rule that cites the section in its evidence already). No link: the files of `render_with_rules`.
  What is not the core of the business is set apart (`issues/done/technical_and_back_office_use_cases.md`): on a feature page, the use cases for which `UseCase::is_technical` holds (the feature is `back_office`, business language under 0.8 or confidence under 0.6; a score not computed yet is no reason) go under "Administration and technical" after the business ones; a `back_office` feature (set by the features pass) is listed under that heading in its domain page and in `llms.txt`, not among the features. Nothing is dropped: the pages and the cache keep them all. The MCP answers keep everything and mark it: `get_feature` and `get_use_case` say so, and a list or a search result carries `[administration/technical]` (no change of ranking). The debt report says how many use cases and back-office features are set apart.
- **retrodoc-mcp** (`corpus.rs`, `search.rs`, `freshness.rs`, `source.rs`, `read_model.rs`, `tools/` (`Docs` in `mod.rs`, one file per tool named after it, the pieces the answers share in `answer.rs`, the tests in `tests.rs`), `server.rs`): read-only access to the generated docs
  for LLM agents, with no LLM call (see `issues/done/mcp_server.md`). `SearchIndex` ranks (the hand-written BM25 of `retrodoc-pipeline`'s `bm25.rs`, shared with the pipeline, title counted
  three times, stop words in English and French dropped, at least half of the query words must match, and those
  matched must hold more than half of the query's IDF weight, `Bm25::search_relevant`, `specs/docs-search.md`, so an entry
  sharing only the common words of a question is left out; the not-documented answer counts those and names the query
  words no entry has) one `Entry` per
  domain, feature, use case, glossary concept and collected Markdown doc, each with its id, confidence and
  cited files. `GeneratedDocs` (`read_model.rs`, `GeneratedDocs::load` reads `.retrodoc/cache/`, business rules and glossary included, + the collected docs) is the typed read model, independent of how it is answered (ADR 0039); `Docs` wraps it with the bounded source access and answers the seven tools in Markdown
  (`list_domains`, `get_domain`, `get_feature`, `get_use_case`, `search_docs`, `read_source`, `git_log`): ids to reuse, confidence (flagged below
  50%), cited files, a stale warning when a cited file no longer matches the hash recorded in `source-hashes.json` (`SourceHashes`, written only by a `generate` that went through every pass, hashes read before the features)
  (`Freshness`, checked at every call; files without a recorded hash are not judged), and "Not documented" as a normal answer for an unknown id or an empty search. `server.rs` puts them
  behind the `rmcp` SDK (`serve_stdio`); tested end to end with an `rmcp` client over an in-memory pipe. `source.rs` (`SourceAccess`) backs `read_source` (a
  window of lines, 200 by default, 400 at most) and `git_log` (hash, date, subject, no author): only files the docs
  cite, never an absolute path, a `..`, a path that is or goes through a symlink (even inside the repo), a binary or a
  file over 1 MiB; a line over 400 characters is cut. `git_log` leaves merge commits out and follows the renames of the file (`specs/file-history.md`), marking the commit
  that renamed it. The two tools run in
  `spawn_blocking`. `SourceAccess` returns typed data (`window`: a `Window` of numbered `Line`s; `commits`: `FileCommit`s;
  a `SourceError` per refusal) that the tools lay out in Markdown and the web interface in HTML; the set of cited
  files is `GeneratedDocs::cited_paths`.
- **retrodoc-web** (`lib.rs` the router and `serve`, `view.rs` what the pages show as plain data, `pages.rs` the
  handlers and their `askama` templates under `templates/`, `assets.rs` the vendored `htmx`, `mermaid` and the stylesheet of
  `assets/`, embedded in the binary and served under `/assets/`, cached an hour; `mermaid` is stored gzipped,
  `mermaid-<version>.min.js.gz` (5.5 MB plain), served with `Content-Encoding: gzip` (406 to a client whose `Accept-Encoding` does not name gzip) and
  cached as immutable under its versioned name): the local web interface of ADR 0039
  (`specs/web-ui.md`). The docs are loaded once into `GeneratedDocs` (`None` before a first `generate`: the home page
  says to run it), and the tree built once; every page shares the layout of `base.html` (`view::Shell`: the name of the
  application in the header and the title, and the tree of domains, features and use cases, linked to
  `/domains/{domain}[/{feature}[/{use_case}]]`); the `Project` that `retrodoc ui` passes to `serve` (then `router`) names the
  application (`ProjectConfig::name_or_directory`: `[project] name` of `retrodoc.toml`, else the repository's
  directory) and carries the purpose and the users of the saved product brief (`commands::brief::saved`: one that can't be read is warned of); a request whose `Host` is a name other than
  `localhost` is refused with 403 (DNS rebinding: an IP address can't be rebound); the home page, headed by the name
  and that purpose, shows the users of the brief, a "Start here" section (`Site::start_links`: the pages of
  `view::REFERENCE_PAGES`, each with what it holds and its count; the header of every page groups the same pages
  under "Read" and "Look up", `Shell::read` and `Shell::look_up`, with what each holds as the title of its link), then the domains as cards with their confidence (flagged low
  under `LOW_CONFIDENCE_THRESHOLD`), a stale badge when a file their features cite changed since the run
  (`Freshness::stale`, read in `spawn_blocking` at each request), and their counts of features and use cases. The page of a domain (`view::domain_page`, `domain.html`) shows the
  sections of its Markdown page in the same order, with a breadcrumb; the back-office features under
  `retrodoc_render::TECHNICAL_HEADING`, the existing docs as links to `/docs/<path>#<anchor>` (the path percent-encoded, `retrodoc_render::anchor`)
  with what a contradicting one states otherwise. The page of a feature (`view::feature_page`, `feature.html`) does
  the same for its Markdown page (the use cases `UseCase::is_technical` sets apart under the same heading, the rules
  with their evidence and doc links, the source files) and adds its entry points, a back-office badge and the stale
  files (read in `spawn_blocking`, like the home page); a feature asked under another domain is not found. The page of a use case (`view::use_case_page`, `use_case.html`)
  follows its Markdown page too: the narrative first (flagged unbacked when `narrative_check` is 0, the rule of the
  debt report), the steps and the diagram folded under "Technical details", or in the open without a narrative; the
  stale files are those its steps cite. Only a page with a diagram loads `mermaid` and `diagrams.js` (the
  `scripts` block of `base.html`), which draw the `<pre class="mermaid">`; the source stays readable under it. The `DocLinks` come
  from `retrodoc ui` (`commands::docs::item_doc_links`: the saved artifacts reconciled with the project docs and the
  saved judgements, no LLM call) through `router(docs, links, collected)`; an unknown slug answers the 404 page. The shared
  pieces of the templates (score badge, breadcrumb, lists, existing docs) are the macros of `macros.html`. The
  templates escape every text of the docs (LLM output). The page of a cited file, `/code/{*path}` (`code::code_page`, `code.html`),
  reads it through the `SourceAccess` of `GeneratedDocs::source_access` (the same refusals and windows as `read_source`):
  `?lines=<start>[-<end>]` shows 10 lines around the cited ones and marks them (rows `id="L<n>"`), `?from=<line>` the
  window from a line (the "Next lines" link, `code::Asked`); a path no doc cites, or not plain, is the 404 page with
  nothing read, any other refusal (a cited range past the end included) a page that says why; then the last 10 commits
  (`SourceAccess::commits`), the features, use cases and rules citing it (`code::citations`, also behind the counts of
  the files page; a rule linked to its `#rule-<slug>` item), and the stale badge. The cited lines of the
  steps, the source files of a feature and the code evidence of a rule link to it (`view::code_href`). The files page,
  `/files` (`files::files_page`, `files.html`, linked from the header of every page), lays out as a tree the paths of the
  files `source-hashes.json` recorded (`Freshness::recorded_paths`) and of the cited ones, built once in `Site::new`: a directory with its files
  cited out of its files, a file with the features, use cases and rules citing it and a link to its page, or `not cited`
  and no link. The page of a collected doc, `/docs/{*path}` (`collected::collected_page`, `collected.html`), renders
  one of the docs `retrodoc ui` collected (`project_docs`, the generated ones left out; served with no run too) with
  `pulldown-cmark`: its raw HTML shown as text, a link or an image of a scheme other than `http`, `https` or `mailto`
  reduced to its text, each heading with the `id` that `anchor` gives its first line without the `#` (the heading the
  doc sections name, so the `/docs/<path>#<anchor>` links land on it). The glossary page, `/glossary`
  (`glossary::glossary_page`, `glossary.html`, linked from the header of every page, built once in `Site::new`), lists
  the concepts of `Glossary::merged_entities` sorted by name ignoring case, each under a heading whose `id` is
  `anchor` of its name, with its definition and the use cases that use it: one of their steps cites a file the concept
  was read from, or a rule of their feature that applies to them names it. Every page has a search box in its header
  (`base.html`, a GET form to `/search`, which `htmx` submits to swap the results into `#search-results`); `/search?q=`
  (`search::search_page`, `search.html`) asks `GeneratedDocs::index` with `retrodoc_mcp::search::SEARCH_LIMIT` (also the
  default `--limit` of `retrodoc search`, so both list the same hits) and groups the hits by kind (domain, feature, use
  case, rule, concept, collected doc), each linked to its page (a rule to `#rule-<slug>` on its feature page, a concept
  to its anchor of the glossary), or gives the not-documented answer with the left-out count and the unknown words; a
  request with `HX-Request` gets `search_results.html` alone, unless it is an `htmx` history restore
  (`HX-History-Restore-Request`, which swaps the whole body; `pages::fragment_or_page`, `Vary` naming both). The rules page, `/rules`
  (`rules::rules_page`, `rules.html`, linked from the header of every page), lists every rule in the order of the run
  with its evidence and a link to its `#rule-<slug>` item (`view::rule_href`), kept by `?kind=` (the artifact name of the kind),
  `?domain=` and `?feature=` (`rules::RuleFilter`, an empty value asks nothing; a rule whose feature is not in the run
  matches no domain nor feature); its form, submitted by `htmx` on change, swaps `rules_list.html` into `#rule-list`
  the same way. The actors page, `/actors` (`actors::actors_page`, `actors.html`, linked from the header of
  every page, built once in `Site::new`), is a table per kind, human then system: a row per actor, grouped by its exact name (a primary actor no
  step names counts as human and starts its use cases in the human row of its name), a column per feature with a use case that names an actor, a cell linking the use cases
  it starts (`starts`) or takes part in, and the permission rules that apply to them. Tests:
  `tests/pages.rs` (home, tree, assets, `Host`), `tests/item_pages.rs` (the item pages), `tests/code_page.rs` (the
  page of a cited file, over a git repo), `tests/files_page.rs`, `tests/doc_pages.rs`, `tests/glossary_page.rs`, `tests/rules_page.rs`, `tests/actors_page.rs` and `tests/search_page.rs` ask the router without a socket (`tower::ServiceExt::oneshot`) over artifacts
  saved in a temp dir. To update `htmx`, replace `assets/htmx.min.js` and the version in `assets.rs`; to update `mermaid`, gzip
  (`gzip -9 -n`) the `dist/mermaid.min.js` of its npm package (checked against the package integrity) into
  `assets/mermaid-<version>.min.js.gz` and change `MERMAID` and the file name in `assets.rs`.
- **retrodoc-cli**: `clap` subcommands (`init`, `scan`, `generate`, `render`, `report`, `roles`, `brief`, `glossary`,
  `entry-points`, `business-read`, `actors`, `surface`, `benchmark`, `benchmark-table`, `search`, `mcp`, `ui`). `generate` runs the whole pipeline then writes the docs (`--dry-run` previews, `--force`
  ignores caches, `--no-confidence` / `--confidence-sample N` / `--max-files N` bound the cost, see the pipeline bullet); `render` writes/previews the docs from the cached artifacts (no LLM call); `report` prints
  the debt report (no LLM call); `business-files [--force]` locates where the business lives (`business-files.yaml`); `contexts [--force]` maps the bounded contexts (`contexts.yaml`, ADR 0029, needs `roles`; `generate` with `domains_from = "contexts"` documents them a slice at a time, `--context a,b` and `--max-files N` choose the slice); `brief [--force] [--signals]` locates the schema and translations (`signal-sources.yaml`), then writes and prints the product brief with where each claim comes from (`--signals`: only how much evidence there is per kind and the sample size, no LLM call); `roles`, `glossary`, `entry-points`, `actors` and `surface` are the standalone phase 7 commands above (`scan`, `roles` and `generate` count as source the files of the languages a saved `roles.yaml` names, `roles::promote_sources`, and say how many, and which extensions of templates, markup, stylesheets or data they left out); `benchmark --reference <file> [--matches <file>] [--judge | --saved-judge] [--out <json>]` prints the figures of the last run and its recall/precision against a hand-written reference (a kind the judge left unjudged printed as such; no LLM call; `--judge` asks the model for pairs and a narrative rating, saved in `.retrodoc/benchmark/judge.yaml`, outside the cache so `generate --force` leaves it; `--saved-judge` scores that saved file again, no LLM call; `--out` saves the figures as JSON, with the scoring version); `benchmark-table <dir> [--previous <dir>]` tabulates saved runs (mean, range, change; no change of recall, precision or judge's rating across scoring versions; a run whose domains or features the judge left unjudged is out of their `(judged)` figures and counted under the series; no LLM call); `search "<query>"` prints the best matches of the lexical search and `mcp` serves the docs to an agent over stdio (neither calls the LLM; both need a `generate` run); `ui [--bind <addr>]` serves the web interface of `retrodoc-web` (`127.0.0.1:8730` by default, `commands::ui::DEFAULT_BIND`; prints the URL once it listens; with no run yet it still starts and its home page says to run `generate`; a non-loopback `--bind` is warned of, the server having no authentication). `main` is `#[tokio::main]` since `generate` awaits the pipeline. Logs (`tracing`) go to stdout, in color only on a terminal: a redirected log is plain text; `mcp` logs to stderr because its stdout carries the protocol. `commands/workspace.rs` is the common start of the commands: `repo_root`, `Workspace::open` (root + `retrodoc.toml`), `Workspace::ingest` (the docs `generate` writes, `<docs_dir>/functional` and `<docs_dir>/_retrodoc`, are left out of `files`, so they never change the tree shape a pass hashed), `source_paths`.

