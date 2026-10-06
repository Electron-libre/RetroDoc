# RetroDoc

Catch up on a software project's documentation debt with LLM agents.

RetroDoc reads a local Git repository (code, commit history, existing Markdown docs) and writes the
**functional documentation** nobody wrote: domains, features, use cases, steps and actors, each with a
Mermaid sequence diagram and a confidence score. It also tells you where the documentation debt is.

> **Status: v1 (MVP).** Functional documentation only, local Git repos only, [OpenRouter](https://openrouter.ai)
> as the LLM provider (any OpenAI-compatible endpoint, such as a local Ollama, works through `base_url`).
> Technical/C4 docs, GitHub/GitLab/Jira connectors and multi-provider support are future work, see
> [PRODUCT.md](./PRODUCT.md) for the vision and [PLAN.md](./PLAN.md) for the roadmap.

## What you get

```
docs/
  functional/
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
export OPENROUTER_API_KEY=...

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
| `roles`, `glossary`, `entry-points`, `actors`, `surface` | Run or inspect one early pipeline stage on its own |

Run `retrodoc <command> --help` for the details.

### Controlling cost on a large repo

`generate` prints the expected number of repo-map LLM calls before it starts. To bound a run:
`--max-files N` keeps only the best-ranked files, `--no-confidence` / `--confidence-sample N` limit the
verification pass, and `llm.concurrency` / `llm.batch_chars` parallelize and batch the summaries.

## Configuration

`retrodoc init` writes a `retrodoc.toml` at the root of the analyzed repo:

```toml
[llm]
provider = "openrouter"
api_key_env = "OPENROUTER_API_KEY"   # name of the variable, never the key itself
model = "anthropic/claude-sonnet-4.5"
# base_url = "http://localhost:11434/v1/chat/completions"   # any OpenAI-compatible server
# reasoning_effort = "none"   # skip "thinking" on models that support it
# timeout_secs = 120
# concurrency = 1
# batch_chars = 6000          # 0 disables batching

[ingest]
extra_ignore = []                              # in addition to .gitignore
existing_docs_paths = ["docs", "README.md"]    # Markdown taken as input
# max_files = 500

[output]
docs_dir = "docs"
```

Intermediate artifacts (file roles, glossary, entry points, domains, features, use cases…) are plain
YAML under `.retrodoc/cache/`. `roles.yaml` is meant to be edited by hand when the LLM misclassifies files.

## How it works

Ingestion → surface extraction (file roles, glossary, entry points) → actors → repo map → domains →
features → use cases → diagrams → business-language score → confidence → Markdown. Each stage is a
Rust module of the `retrodoc-pipeline` crate with its rationale in its `//!` doc; the overview is in
[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md), and the design decisions are recorded in
[docs/adr/](./docs/adr).

The workspace has six crates with one-way dependencies: `retrodoc-cli`, `retrodoc-pipeline`,
`retrodoc-ingest`, `retrodoc-llm`, `retrodoc-render`, `retrodoc-core`.

## Development

```sh
just check          # fmt + clippy -D warnings + tests
just test-harness   # test the agent hooks and skills (needs rust-script)
```

CI (`.github/workflows/ci.yml`) runs the same checks plus the harness tests, and validates the commit messages of pull requests.

Commit messages follow Conventional Commits, see [AGENTS.md](./AGENTS.md). Agent-specific guidance is in
[CLAUDE.md](./CLAUDE.md).

## License

[MIT](./LICENSE).
