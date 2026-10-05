---
name: update-docs
description: Checklist-driven update of RetroDoc's project documentation (CLAUDE.md, PLAN.md, PRODUCT.md, docs/ARCHITECTURE.md, AGENTS.md, doc comments) after a code change, a finished roadmap phase, or a new command/config key. Use when finishing a feature, before committing, or when asked to sync/update/audit the docs, so nothing is forgotten.
---

# Update the project documentation

Goal: after a change, every document that describes the touched behavior says the same true thing.
Work through the steps in order and report the result of each one. Don't skip a step because it
"probably doesn't apply": check it, then write "n/a" with the reason.

## 1. Establish what changed

- Run `git status` and `git diff` (staged + unstaged), and `git log -5 --format=%s` for recent context.
- List the changes in one line each, classified as: new/changed **command or flag**, **config key**
  (`retrodoc.toml`), **pipeline pass or artifact** (`.retrodoc/cache/*`), **crate/module**, **dependency
  direction**, **roadmap phase status**, **convention** (lint, testing, commit format).
- If the diff is empty, ask which change or commit range to document instead of guessing.

## 2. Map each change to the documents to touch

| Change | Update |
|---|---|
| New/renamed/removed CLI command or flag | `CLAUDE.md` (Commands + `retrodoc-cli` bullet), `docs/ARCHITECTURE.md` §3 (commands), `PLAN.md` §4 (`retrodoc-cli` list) |
| New pipeline pass or changed pass behavior | `CLAUDE.md` (`retrodoc-pipeline` bullet), `docs/ARCHITECTURE.md` §4 (pipeline, safety nets, confidence) and §5 (incremental re-run) if caching changed, `PLAN.md` §2 |
| New artifact or cache file under `.retrodoc/` | `CLAUDE.md`, `docs/ARCHITECTURE.md` §7 (what lands on disk) |
| Output files written to the docs dir | `docs/ARCHITECTURE.md` §6, `PLAN.md` §3 (expected output), `CLAUDE.md` (`retrodoc-render` bullet) |
| New/changed `retrodoc.toml` key | `retrodoc-core` doc comments in `config.rs`, the default file written by `Config::write_default` (used by `retrodoc init`), any doc showing a sample config |
| New crate, moved module, changed dependency direction | `CLAUDE.md` architecture diagram + bullets, `docs/ARCHITECTURE.md` §2, `PLAN.md` §4 |
| LLM provider / boundary change | `docs/ARCHITECTURE.md` §8, `CLAUDE.md` (`retrodoc-llm` bullet), `PLAN.md` scope and risks if v1 limits move |
| Roadmap phase started, finished, or re-scoped | `PLAN.md` §5 (remove "Not started" / add status) and the matching §7.x section, `CLAUDE.md` if it lists what is implemented |
| New or changed convention (lint, tests, commit format, language) | `CLAUDE.md` "Conventions", `AGENTS.md` for commit rules |
| Product-level scope change (new connector, doc type) | `PRODUCT.md`, `PLAN.md` §1 scope |
| Public Rust API added or changed | doc comments: `# Errors` section on `pub fn` returning `Result`, `#[must_use]` on pure getters (clippy pedantic) |

## 3. Detect stale statements (don't trust memory)

For every command, flag, config key, artifact name and module you touched, grep the docs for the
old and the new name and read each hit in context:

```sh
rg -n "<name>" CLAUDE.md PLAN.md PRODUCT.md AGENTS.md docs/
```

Also compare these lists against the code, since they drift first:

- Subcommands in `crates/retrodoc-cli/src/main.rs` and `commands/` vs. the command lists in `CLAUDE.md`,
  `docs/ARCHITECTURE.md` §3 and `PLAN.md` §4.
- Modules in `crates/retrodoc-pipeline/src/` vs. the passes described in `CLAUDE.md` and
  `docs/ARCHITECTURE.md`.
- Phase statuses in `PLAN.md` §5 vs. `git log` (done phases must not say "Not started").
- Cross-references (`§N`, relative links, file paths) still resolve after any heading or file rename.

## 4. Edit

- Change only what is now wrong or missing; keep each document's existing tone, density and structure.
- Everything is in English (see `CLAUDE.md` conventions).
- `docs/ARCHITECTURE.md` is a high-level map with diagrams: keep it short, update Mermaid diagrams when
  the flow changes, and don't copy per-module detail into it.
- `PLAN.md` is the source of truth for roadmap and scope; `CLAUDE.md` describes the current
  implementation. Don't state the same fact in a third place.
- Don't invent behavior: if you can't confirm it from the code or the diff, read the code or leave it out.

## 5. Verify

- `cargo fmt --all` and `cargo clippy --workspace --all-targets` must stay warning-free if doc comments
  or code changed.
- `cargo test --workspace` if code changed along with the docs.
- Re-run the grep from step 3 for old names: no hit should remain unless intentionally historical.
- Check Mermaid blocks are syntactically balanced and relative links point to existing files.

## 6. Report

Finish with a short table: document → updated / n/a (reason) → what changed. Call out anything
you could not verify or deliberately left alone.

Don't commit unless asked. When asked, follow `AGENTS.md` (`docs(<scope>): <description>`, subject
72 characters max, no AI attribution).
