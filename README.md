# RetroDoc

Catch up on a software project's documentation debt with LLM agents.

RetroDoc reads a local Git repository (code, commit history, existing Markdown docs) and writes the
**functional documentation** nobody wrote: domains, features, use cases, steps and actors, each with a
Mermaid sequence diagram and a confidence score. It also tells you where the documentation debt is.

> **Status: v1 (MVP).** Functional documentation only, local Git repos only, [OpenRouter](https://openrouter.ai)
> or the [DeepSeek](https://platform.deepseek.com) API as the LLM provider (any OpenAI-compatible endpoint, such as a
> local Ollama, works through `base_url`).
> Technical/C4 docs, GitHub/GitLab/Jira connectors and multi-provider support are future work, see
> [PRODUCT.md](./PRODUCT.md) for the vision and [PLAN.md](./PLAN.md) for the roadmap.

## What you get

```
docs/
  functional/
    llms.txt                             # index for agents: domains, features, confidence
    <domain>/
      README.md                          # domain overview, confidence
      <sub-domain>/
        <feature>.md                     # list of use cases
        use-cases/<feature>/<use-case>.md  # business narrative, steps, actors, diagram, confidence
  _retrodoc/
    coverage-report.md                   # gaps and low-confidence sections
    run-metadata.json                    # model, date, analyzed commit
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
| `scan` | Summarize the repo (files, history, docs) without writing |
| `generate` | Run the whole pipeline and write the docs (`--dry-run`, `--force`, `--no-confidence`, `--confidence-sample N`, `--max-files N`) |
| `render` | Write the docs from the cached artifacts of the last run, no LLM call |
| `report` | Print the documentation debt report, no LLM call |
| `mcp` | Serve the generated docs to LLM agents (Claude Code, Cursor…) as a read-only MCP server over stdio, no LLM call. Point the agent at `retrodoc mcp --path <repo>` |
| `search` | Search the generated docs lexically, no LLM call (what the MCP server's `search_docs` finds) |
| `brief` | Write the product brief (what the application does, for whom, its main objects and capabilities) from docs, commits, manifests, schema, translations and tests (`--force`, `--signals` to only count the evidence, no LLM call) |
| `roles`, `business-files`, `contexts`, `glossary`, `entry-points`, `actors`, `surface` | Run or inspect one early pipeline stage on its own |

Run `retrodoc <command> --help` for the details.

### Controlling cost on a large repo

With `domains_from = "files"`, `generate` prints the expected number of repo-map LLM calls before it starts
(the default `surface` path makes no repo map). To bound a run:
`--max-files N` keeps only the best-ranked files (on the `surface` path the features still come from every
entry point; with `contexts` it is the reading budget of a run), `--no-confidence` / `--confidence-sample N` limit the
verification pass, and `llm.concurrency` / `llm.batch_chars` parallelize and batch the summaries (`llm.concurrency` also the use cases and the confidence checks).

## Configuration

`retrodoc init` writes a `retrodoc.toml` at the root of the analyzed repo:

```toml
[llm]
provider = "openrouter"             # or "deepseek" (model "deepseek-chat", key in DEEPSEEK_API_KEY)
api_key_env = "OPENROUTER_API_KEY"   # name of the variable, never the key itself
model = "anthropic/claude-sonnet-4.5"
# base_url = "http://localhost:11434/v1/chat/completions"   # any OpenAI-compatible server
# reasoning_effort = "none"   # skip "thinking" on models that support it
# temperature = 0             # sampling temperature; unset: 0 for every pass but contexts, repo-map and the benchmark judge (the server's default there); a server that refuses it is asked again without
# timeout_secs = 120          # unset: 120, 900 for a loopback base_url (a local server)
# concurrency = 1
# batch_chars = 6000          # 0 disables batching
# structured_output = true    # ask the server for schema-constrained JSON answers (false: never)

# A pass can use another model or server: `[llm.passes.<name>]` takes the keys above and falls back to
# `[llm]` for the rest. Names: sources, brief, roles, business-files, contexts, glossary, entry-points, actors,
# repo-map, domains, features, use-cases, confidence, benchmark judge.
# [llm.passes.domains]
# model = "anthropic/claude-sonnet-4.5"
# base_url = ""               # empty: the provider's own endpoint instead of [llm]'s base_url

[ingest]
extra_ignore = []                              # in addition to .gitignore
existing_docs_paths = ["docs", "README.md"]    # Markdown taken as input
# max_files = 500

[output]
docs_dir = "docs"

[pipeline]
domains_from = "surface"  # "surface" (default, ADR 0034): domains from the entities and entry points (no repo map; the features come from the entry points); "files": from the file summaries; "contexts": a large application documented a bounded context at a time (see below)
rules = true              # state the business rules of each feature; false: a cheaper run (the rules are about 40% of the input tokens)
# business_read = false   # each pass reads the business files alone: for a small library, where the joint reading finds fewer entry points

# [rules]                 # put back what the rules pass set aside by mistake (listed in the debt report); kept by --force
# keep_features = ["email-verification"]    # not features about tests: their rules are stated
# keep_rules = ["orders-2"]                 # rules dropped as noise, published anyway (no LLM call)
```

**A large application, in several runs.** With `domains_from = "contexts"`, `retrodoc contexts` (or the first
`generate`) maps the application into bounded contexts from the names of its entry point files and one LLM call
(`contexts.yaml`, editable: move a group or a folder, rename a context); each `generate` then documents the next
contexts not done, as many as fit `--max-files` (default 150 files; a context is never cut), or the ones you
name with `--context a,b`. A run reads only its slice, never redoes or changes the contexts already
documented (a rerun with nothing left to do calls no LLM), and the docs gather every context done; the output
lists the contexts left. An interrupted run resumes where it stopped.

A pass whose model changes is redone on the next run (the model is part of what its cache is compared
with); the first run after upgrading to this behavior redoes every pass once. `roles.yaml`,
`signal-sources.yaml`, `business-files.yaml`, `contexts.yaml` and `product.yaml` are the exception: they stay until `--force`.
A suggested split, **not benchmarked yet**: the strong model on `brief`, `domains` and `features`, which
frame everything else, and a cheap or local one in `[llm]` for the many extraction calls (`repo-map`,
`glossary`, `entry-points`, `actors`, `confidence`). Measure it against one model for all with
`just benchmark` (the `quality-benchmark` skill: quality and cost, which the recap splits per pass and model).

Intermediate artifacts (file roles, glossary, entry points, domains, features, use cases…) are plain
YAML under `.retrodoc/cache/`. `roles.yaml`, `signal-sources.yaml` and `product.yaml` are meant to be edited by hand when the LLM gets them wrong.

## How it works

Ingestion → surface extraction (file roles, glossary, entry points) → actors → repo map (`files` path only) → domains →
features → use cases → diagrams → business-language score → confidence → Markdown. Each stage is a
Rust module of the `retrodoc-pipeline` crate with its rationale in its `//!` doc; the overview is in
[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md), and the design decisions are recorded in
[docs/adr/](./docs/adr).

The workspace has seven crates with one-way dependencies: `retrodoc-cli`, `retrodoc-pipeline`,
`retrodoc-ingest`, `retrodoc-llm`, `retrodoc-render`, `retrodoc-mcp`, `retrodoc-core`.

## Development

```sh
just check                # fmt + clippy -D warnings + tests + specs + check-harness (needs `harness`, see docs/dev/agent-harness.md)
just test-skill-scripts   # test the scripts of the smoke-test and quality-benchmark skills (needs rust-script)
```

CI runs the Rust checks and the tests of the skill scripts (`.github/workflows/ci.yml`) and, from the shared harness, the specs, the synced files and the commit messages of pull requests (`.github/workflows/harness.yml`).

Commit messages follow Conventional Commits, see [AGENTS.md](./AGENTS.md). Agent-specific guidance is in
[CLAUDE.md](./CLAUDE.md).

## License

[MIT](./LICENSE).
