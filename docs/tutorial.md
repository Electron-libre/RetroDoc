# Tutorial: document an application with RetroDoc

This tutorial takes you from a Git repository with no functional documentation to a set of Markdown pages that
say what the application does, a report of where the documentation is weak, and the tools to read, question and
correct them. It takes about fifteen minutes plus the time of the LLM run.

The examples come from a real run on [linkding](https://github.com/sissbruecker/linkding), an open-source bookmark
manager (Django). Your figures will differ.

## 1. Before you start

You need:

- a local clone of the repository to document, with its Git history (RetroDoc reads the commits);
- a Rust toolchain ([rustup](https://rustup.rs)); the version is pinned in `rust-toolchain.toml` and installed on
  the first build;
- an API key for [OpenRouter](https://openrouter.ai) or [DeepSeek](https://platform.deepseek.com), or a local
  OpenAI-compatible server (Ollama, llama.cpp).

Build the binary:

```sh
git clone <the RetroDoc repository> && cd RetroDoc
cargo build --release
export PATH="$PWD/target/release:$PATH"   # or call target/release/retrodoc
```

Every command takes `--path <repo>` (default: the current directory) and nothing else is required. In this
tutorial, `~/src/linkding` is the repository to document.

## 2. Create the configuration

```sh
retrodoc init --path ~/src/linkding
```

This writes `retrodoc.toml` at the root of the repository, every option commented. Pick the provider:

```toml
[llm]
provider = "deepseek"            # or "openrouter" (default)
model = "deepseek-chat"
api_key_env = "DEEPSEEK_API_KEY" # the name of the variable, never the key itself
```

Then give the key, in the environment or in a `.env` file of the directory you run `retrodoc` from (or a parent):

```sh
export DEEPSEEK_API_KEY=sk-...
```

With a local server, set `base_url` instead, for example
`base_url = "http://localhost:11434/v1/chat/completions"`. A local model is free but much slower: count hours on a
medium application.

Two settings worth deciding now:

- `[output] language = "French"`: the language of the generated pages (unset: English in practice);
- `[pipeline] rules = false`: skip the business rules for a cheaper first run (they are about 40% of the input
  tokens).

## 3. Look before you spend

`scan` reads the repository without any LLM call and writes nothing:

```console
$ retrodoc scan --path ~/src/linkding
Repo: /home/me/src/linkding
Files: 443 (source: 178, test: 82, markdown: 19, other: 164)
Git history: 642 file(s) with at least one commit
Existing Markdown docs taken as input: 0
```

If a source directory is missing or vendored code is counted, add it to `[ingest] extra_ignore`; `.gitignore` is
already honoured. `existing_docs_paths` lists the Markdown taken as input (default: `docs` and `README.md`).

Then ask for the estimate of the run, still with no LLM call:

```sh
retrodoc generate --path ~/src/linkding --estimate
```

It prints the calls and the tokens of each pass, as a range drawn from runs on other applications. Tokens only:
multiply by the price of your model. On a large application, bound the run with `--max-files N` (the best-ranked
files) or `--no-confidence` (skips the verification pass); see section 9 for very large ones.

## 4. Generate the docs

Preview first: `--dry-run` runs the pipeline and prints the files and diffs it would write, without writing them.

```sh
retrodoc generate --path ~/src/linkding --dry-run
retrodoc generate --path ~/src/linkding
```

The second call reuses the LLM answers of the first (they are cached under `.retrodoc/cache/`), so previewing
costs nothing extra. The recap at the end gives the calls and tokens of each pass against the estimate.

An interrupted run (Ctrl-C, network error) resumes where it stopped: run the same command again.

## 5. Read what was written

```
docs/
  functional/
    llms.txt                             # index: domains, features, confidence
    <domain>/
      README.md                          # the domain, its features
      <feature>.md                       # the feature, its use cases
      use-cases/<feature>/<use-case>.md  # narrative, steps, actors, diagram
  _retrodoc/
    coverage-report.md                   # the documentation debt
    run-metadata.json                    # model, date, analyzed commit
```

Start with `docs/functional/llms.txt`: one line per domain and feature with its confidence. On linkding:

```
- [Bookmark Management](bookmark-management/README.md) (95%): Saving, editing, searching, filtering, archiving, ...
  - [Bookmark REST API](bookmark-management/bookmark-rest-api.md) (96%): Developers and external tools list, ...
  - [Bookmark import and export](bookmark-management/bookmark-import-export.md) (85%): Users import bookmarks ...
```

A use case page reads in business terms first, with the technical detail folded below:

```markdown
# Merge tags into a target tag

The bookmark owner picks a target tag and one or more tags to merge into it; the bookmarks are reassigned and the
merged tags are removed.

**Confidence:** 100% — all steps supported by the cited code
**Business language:** 100%
**Primary actor:** Bookmark owner

## What happens
The bookmark owner has several overlapping labels and wants to consolidate them. ...

<details><summary>Technical details (4 steps)</summary>
1. **Bookmark owner** (human) — Opens the merge form and submits a target tag plus one or more tags to merge.
   - Code: `bookmarks/views/tags.py:113-115`
2. **System** (system) — Validates that at least one merge tag is given, ...
   - Code: `bookmarks/forms.py:189-212`
...
(Mermaid sequence diagram)
</details>
```

How to read the two scores:

- **Confidence**: how well the cited code supports each step, checked mechanically and by the LLM. Below 50% the
  page is marked ⚠️: treat it as a lead to check against the code, not as a fact.
- **Business language**: a deterministic score that drops when the narrative paraphrases the code (class names,
  HTTP verbs) instead of saying what the user does.

## 6. Find the documentation debt

```sh
retrodoc report --path ~/src/linkding
```

No LLM call. The report (also written to `docs/_retrodoc/coverage-report.md`) lists, among others:

- the confidence of each domain, weakest first;
- the sections below 50%, with the step that the code does not support;
- the narratives the cited code contradicts;
- the features set aside as technical (backups, health checks, dev tooling), absent from the functional docs.

On linkding it flags, for example, `api-access/api-token-management/delete-api-token` at 25% ("No delete view or
form handling shown in provided code") and a narrative claiming assets can be created through a REST endpoint that
does not allow it. These are the pages to read first.

## 7. Browse, review and correct in the web interface

```sh
retrodoc ui --path ~/src/linkding
# Serving the docs at http://127.0.0.1:8730/ (Ctrl+C to stop)
```

No LLM call. Besides the pages, it shows a map of the domains, the cited code and its history, the files no page
cites, the glossary, the actors, the debt ranked by priority, and the history of the runs.

Each domain, feature and use case has two forms:

- **Review**: mark it validated, wrong or to review (saved in `.retrodoc/reviews.json`). A section marked wrong
  enters the debt report; a validated one leaves it until its text is rewritten or its code changes, when the
  review is flagged as outdated.
- **Edit**: correct the name, the description or the narrative (saved in `.retrodoc/edits.json`). Your text is
  shown instead of the generated one, in the web interface, in `retrodoc render` and through the MCP server, marked
  with its author and date; one button brings the generated text back.

After editing, `retrodoc render --path ~/src/linkding` writes the Markdown again from the saved artifacts, with no
LLM call.

## 8. Question the docs

**Search**, lexically, no LLM call:

```console
$ retrodoc search "tag" --path ~/src/linkding --limit 2
[use case] Merge tags into a target tag (tagging-and-bundles/tag-management/merge-tags, confidence 1.00, score 2.26)
    The bookmark owner picks a target tag and one or more tags to merge into it; ...
    files: bookmarks/views/tags.py, bookmarks/forms.py
[feature] Tag management (tagging-and-bundles/tag-management, confidence 1.00, score 2.24)
    ...
```

**Ask** a question: an LLM agent reads the docs (and the code and history they cite) through read-only tools,
then answers with its sources, and warns about the low-confidence ones and those whose code changed since the run.

```sh
retrodoc ask "Who can see a shared bookmark?" --path ~/src/linkding
retrodoc chat --path ~/src/linkding           # one question per line, follow-ups keep the context; `exit` ends it
retrodoc ui --path ~/src/linkding --ask       # the same, in the web interface
```

The model of these commands is set in `[llm.passes.ask]` (falls back to `[llm]`).

**Give the docs to your coding agent** through the MCP server (read-only, no LLM call). With Claude Code:

```sh
claude mcp add retrodoc -- retrodoc mcp --path /home/me/src/linkding
```

Any MCP client works the same way: it launches `retrodoc mcp --path <repo>` and talks over stdio. The agent then
searches and reads domains, features and use cases, sees the human reviews and edits, and is warned when the code
a page cites has changed since the run.

## 9. Keep the docs up to date

Rerun `generate` after the code changes:

```sh
retrodoc generate --path ~/src/linkding
```

Only what changed goes back to the LLM; a rerun with nothing changed calls no LLM and rewrites nothing. A pass
whose model you change in the config is redone. `--force` redoes everything from scratch.

**Steer the analysis.** The early passes save their answers as YAML under `.retrodoc/cache/`, and some are meant
to be corrected by hand when the LLM gets them wrong; they are kept until `--force`:

| File | What to correct |
|---|---|
| `roles.yaml` | the stack and the role of each file (entry point, model, logic, infra…) |
| `product.yaml` | the product brief: what the application does, for whom |
| `business-files.yaml` | where the business code lives |
| `contexts.yaml` | the bounded contexts of a large application (below) |

Each early pass can also be run and inspected on its own: `retrodoc brief`, `roles`, `business-files`,
`glossary`, `entry-points`, `actors`, `surface` (see `retrodoc <command> --help`).

**A large application, in several runs.** Set `[pipeline] domains_from = "contexts"`. `retrodoc contexts` maps the
application into bounded contexts (`contexts.yaml`, editable); each `generate` then documents the next contexts
not done, up to `--max-files` (150 files by default), or the ones you name with `--context orders,billing`. The
output lists the contexts left; run `generate` again until none is.

**Split the models.** `[llm.passes.<name>]` gives a pass its own model or server, for example a strong model on
`brief`, `domains` and `features` and a cheap or local one in `[llm]` for the rest. The list of passes is in the
comments of `retrodoc.toml`.

## Where to go next

- [README.md](../README.md): every command and configuration key at a glance.
- `retrodoc <command> --help`: the options of each command.
- [ARCHITECTURE.md](ARCHITECTURE.md): how the pipeline works, what lands on disk and why.
