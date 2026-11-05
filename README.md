# RetroDoc

Catch up on a software project's documentation debt with LLM agents.

RetroDoc reads a local Git repository (code, commit history, existing Markdown docs) and writes the
**functional documentation** nobody wrote: domains, features, use cases, steps and actors, each with a
Mermaid sequence diagram and a confidence score. It also tells you where the documentation debt is.

> **Status: v1 (MVP).** Functional documentation only, local Git repos only, [OpenRouter](https://openrouter.ai)
> or the [DeepSeek](https://platform.deepseek.com) API as the LLM provider (any OpenAI-compatible endpoint, such as a
> local Ollama, works through `base_url`).
> Technical/C4 docs, GitHub/GitLab/Jira connectors and multi-provider support are future work, see
> [PRODUCT.md](./PRODUCT.md) for the vision and [ROADMAP.md](./ROADMAP.md) for the roadmap.

## What you get

```
docs/
  functional/
    llms.txt                             # index for agents: domains, features, confidence
    <domain>/
      README.md                          # domain overview, confidence
      <feature>.md                       # list of use cases
      use-cases/<feature>/<use-case>.md  # business narrative, steps, actors, diagram, confidence
  _retrodoc/
    coverage-report.md                   # gaps and low-confidence sections
    run-metadata.json                    # model, date, analyzed commit
    rendered-pages.json                  # hash of each page written, to remove the pages no run produces any more
```

- **Grounded**: use cases are built from the code that actually runs (routes, handlers, models), and each
  step cites it. A cross-check pass asks the LLM to verify every step against the cited code.
- **Business language**: the output describes what users do, not what the code does. A deterministic
  score flags use cases that read like a paraphrase of the code.
- **Safe to rerun**: unchanged units are not sent to the LLM again, an interrupted run resumes where it
  stopped, and rewriting identical docs is a no-op. `--dry-run` previews a diff before anything is written.

## Install

RetroDoc is a Rust workspace; the toolchain is pinned in `rust-toolchain.toml` (rustup installs it on first build), bump it deliberately.

```sh
git clone <this repo> && cd RetroDoc
cargo build --release          # binary: target/release/retrodoc
```

## Quick start

```sh
export OPENROUTER_API_KEY=...     # or DEEPSEEK_API_KEY with provider = "deepseek"; a .env file works too

retrodoc init     --path <target-repo>   # creates retrodoc.toml
retrodoc scan     --path <target-repo>   # files + history + existing docs, no LLM, nothing written
retrodoc generate --path <target-repo> --dry-run   # run the pipeline, preview the diff
retrodoc generate --path <target-repo>             # write the docs
retrodoc report   --path <target-repo>   # documentation debt report, no LLM
```

During development, replace `retrodoc` with `cargo run -p retrodoc-cli --`.

### Commands

| Command | Purpose |
|---|---|
| `init` | Create a default `retrodoc.toml` |
| `scan` | Summarize the repo (files, history, docs) without writing; the languages a saved `roles.yaml` names count as source |
| `generate` | Run the whole pipeline and write the docs (`--dry-run`, `--estimate`, `--force`, `--no-confidence`, `--confidence-sample N`, `--max-files N`) |
| `render` | Write the docs from the cached artifacts of the last run, no LLM call; a section a human edited in `retrodoc ui` shows the edited text, marked with its author and date |
| `report` | Print the documentation debt report, with the sections a human marked wrong in `retrodoc ui`, no LLM call |
| `mcp` | Serve the generated docs to LLM agents (Claude Code, Cursor…) as a read-only MCP server over stdio, no LLM call; a domain, feature or use case gives its human review, says when the review is outdated (the section rewritten or its code changed since) and warns when it was marked wrong; an edited section says it is a human edit, and an outdated edit ends with the text the last run generated. Point the agent at `retrodoc mcp --path <repo>` |
| `ui` | Browse and search the generated docs in a local web interface (a home page that says where to start, a map of the domains, domains, confidence, stale code, the cited code and its history, the files no doc cites, the glossary, the business rules, the actors, the project's own docs, the debt of the docs to audit (with the code no doc covers and what the project's own docs contradict or leave out), ranked by a priority that crosses the weight of its kind and how weak each item is with the churn of its busiest file and its entry points and a dashboard of where it concentrates and how the figures moved across the runs), the history of the runs, with the sections a `generate` added, removed or made worse, and a form on each domain, feature and use case to mark it validated, wrong or to review (saved in `.retrodoc/reviews.json`; a validated section leaves the debt of judgement until its section is rewritten or its code changes, one marked wrong or to review enters it, an outdated review is flagged, an edit whose section was rewritten or whose code changed since enters it and is flagged on its page with the new generated text, and the dashboard shows the share reviewed), and a form to correct its name, description and narrative (saved in `.retrodoc/edits.json`, shown over the generated text, which one button brings back, and marked edited in the lists), no LLM call: `retrodoc ui --path <repo> [--bind 127.0.0.1:8730]` prints the URL to open |
| `ask` | Answer a question about the application from the generated docs: the LLM reads them with the read-only tools of the MCP server (and the code and history they cite), then the answer is printed with the domains, features, use cases and files it rests on, each checked against what the tools showed, and warns of those with a low confidence or whose code changed since the docs were generated (`--max-rounds N`, 8 by default; settings in `[llm.passes.ask]`); writes only its usage. `retrodoc ask "Who can cancel an order?" --path <repo>` |
| `chat` | Ask questions one after the other, each answered like `ask` after the earlier questions and answers, so a follow-up need not restate its context; one question per line, `exit`, `quit` or the end of the input ends it (`--max-rounds N` per question; settings in `[llm.passes.ask]`); writes only its usage. `retrodoc chat --path <repo>` |
| `search` | Search the generated docs lexically, no LLM call (what the MCP server's `search_docs` finds) |
| `brief` | Write the product brief (what the application does, for whom, its main objects and capabilities) from docs, commits, manifests, schema, translations and tests (`--force`, `--signals` to only count the evidence, no LLM call) |
| `roles`, `business-files`, `contexts`, `glossary`, `entry-points`, `actors`, `surface` | Run or inspect one early pipeline stage on its own |

Run `retrodoc <command> --help` for the details.

### Controlling cost on a large repo

Before its first LLM call, `generate` prints an estimate of the calls and tokens of each pass and in total, a range
extrapolated from runs measured on other applications (a rerun with nothing changed is estimated at zero);
`generate --estimate` prints it and stops, with no LLM call and nothing written. Tokens only, no price. The recap
at the end of the run sets each pass against its range (within it, or the miss in percent), and `.retrodoc/cache/usage.json`
keeps both.
To bound a run: `--max-files N` keeps only the best-ranked files (the features still come from every
entry point; with `contexts` it is the reading budget of a run), `--no-confidence` / `--confidence-sample N` limit the
verification pass, and `llm.concurrency` parallelizes the use cases, the rules and the confidence checks.

## Configuration

`retrodoc init` writes a `retrodoc.toml` at the root of the analyzed repo:

```toml
# [project]
# name = "Shop"                     # the name the web interface shows; unset: the name of the repository's directory

[llm]
provider = "openrouter"             # or "deepseek" (model "deepseek-chat", key in DEEPSEEK_API_KEY)
api_key_env = "OPENROUTER_API_KEY"   # name of the variable, never the key itself
model = "anthropic/claude-sonnet-4.5"
# base_url = "http://localhost:11434/v1/chat/completions"   # any OpenAI-compatible server
# reasoning_effort = "none"   # skip "thinking" on models that support it
# temperature = 0             # sampling temperature; unset: 0 for every pass but the benchmark judge (the server's default there); a server that refuses it is asked again without
# timeout_secs = 120          # unset: 120, 900 for a loopback base_url (a local server)
# concurrency = 1
# structured_output = true    # ask the server for schema-constrained JSON answers (false: never)

# A pass can use another model or server: `[llm.passes.<name>]` takes the keys above and falls back to
# `[llm]` for the rest. Names: sources, brief, roles, business-files, contexts, glossary, entry-points, actors,
# domains, features, use-cases, rules, confidence, reconcile, benchmark judge, ask.
# [llm.passes.domains]
# model = "anthropic/claude-sonnet-4.5"
# base_url = ""               # empty: the provider's own endpoint instead of [llm]'s base_url

[ingest]
extra_ignore = []                              # in addition to .gitignore
existing_docs_paths = ["docs", "README.md"]    # Markdown taken as input
# max_files = 500

[output]
docs_dir = "docs"
# language = "French"  # the language of the generated docs: a name, or en, fr, de, es, it, pt, nl; unset: the model picks (English in practice). Changing it redoes the writing passes; the headings of the docs and the labels of the web interface are in French for French (fr, French, Français or a regional tag such as fr-FR, in any case and accents), English for any other

[pipeline]
domains_from = "surface"  # "surface" (default): domains from the entities and entry points, features from the entry points; "contexts": a large application documented a bounded context at a time (see below). "files" was removed (ADR 0035)
rules = true              # state the business rules of each feature; false: a cheaper run (the rules are about 40% of the input tokens)
# business_read = false   # each pass reads the business files alone: for a small library, where the joint reading finds fewer entry points

# [rules]                 # put back what the rules pass set aside by mistake (listed in the debt report); kept by --force
# keep_features = ["email-verification"]    # not features about tests: their rules are stated
# keep_rules = ["orders-2"]                 # rules dropped as noise, published anyway (no LLM call)
```

**A large application, in several runs.** With `domains_from = "contexts"`, `retrodoc contexts` (or the first
`generate`) maps the application into bounded contexts from the names of its entry point files and one LLM call
(`contexts.yaml`, editable: move a group, a folder or a file, rename a context); each `generate` then documents the next
contexts not done, as many as fit `--max-files` (default 150 files; a context is never cut), or the ones you
name with `--context a,b`. A run reads only its slice, never redoes or changes the contexts already
documented (a rerun with nothing left to do calls no LLM), and the docs gather every context done; the output
lists the contexts left. An interrupted run resumes where it stopped. The technical files (roles `infra` and
`config`) are in no context; the files of a context made of layer folders (UI components, models) go to the contexts
that name them, and a catch-all context (`misc`, `other`) is spread the same way. `retrodoc contexts --force` maps
again, the saved support proposed to the call: a new context that holds most of the files of
a saved one takes its slug and name, so a context documented stays done while its files and the files read for it did not change.

A pass whose model changes is redone on the next run (the model is part of what its cache is compared
with); the first run after upgrading to this behavior redoes every pass once. `roles.yaml`,
`roles-unmatched.yaml`, `signal-sources.yaml`, `business-files.yaml`, `contexts.yaml` and `product.yaml` are the exception: they stay until `--force`.
A suggested split, **not benchmarked yet**: the strong model on `brief`, `domains` and `features`, which
frame everything else, and a cheap or local one in `[llm]` for the many extraction calls (`roles`,
`glossary`, `entry-points`, `actors`, `confidence`). Measure it against one model for all with
`just benchmark` (the `quality-benchmark` skill: quality and cost, which the recap splits per pass and model).

Intermediate artifacts (file roles, glossary, entry points, domains, features, use cases…) are plain
YAML under `.retrodoc/cache/`. `roles.yaml`, `roles-unmatched.yaml`, `signal-sources.yaml` and `product.yaml` are meant to be edited by hand when the LLM gets them wrong.

## How it works

Ingestion → surface extraction (file roles, glossary, entry points) → actors → domains →
features → use cases → diagrams → business-language score → confidence → Markdown. Each stage is a
Rust module of the `retrodoc-pipeline` crate with its rationale in its `//!` doc; the overview is in
[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md), and the design decisions are recorded in
[docs/adr/](./docs/adr).

The workspace has ten crates with one-way dependencies: `retrodoc-cli`, `retrodoc-pipeline`,
`retrodoc-bench`, `retrodoc-agent`, `retrodoc-ingest`, `retrodoc-llm`, `retrodoc-render`, `retrodoc-mcp`, `retrodoc-web`, `retrodoc-core`.

## Development

```sh
just check                # fmt + clippy -D warnings + tests + specs + check-cardinal (needs `cardinal`, see docs/dev/agent-harness.md)
just test-skill-scripts   # test the scripts of the smoke-test and quality-benchmark skills (needs rust-script)
```

CI runs the Rust checks and the tests of the skill scripts (`.github/workflows/ci.yml`) and, from Cardinal (the shared harness), the specs, the synced files and the commit messages of pull requests (`.github/workflows/cardinal.yml`).

Commit messages follow Conventional Commits, see [.claude/skills/commit-message/conventional-commits.md](./.claude/skills/commit-message/conventional-commits.md). Agent-specific guidance is in
[CLAUDE.md](./CLAUDE.md).

## License

[MIT](./LICENSE).
