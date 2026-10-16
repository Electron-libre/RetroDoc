# Stop an unchanged rerun from rewriting files that hold a timestamp

# Goal
Priority: low. A second `generate` that reuses everything from the cache still writes files: it must write
none, so that the smoke test criterion "the rerun is a no-op" holds on a large application.

# Findings (smoke run of 2026-10-15, a large Rails application, local model)
* The rerun made no LLM call ("everything was reused from the cache") but reported "3 file(s) written to docs".
* The diff shown in the log was a `generated_at` timestamp (next to a `commit` field) that changed between
  the two runs.
* Which three files, and whether other generated files carry a timestamp, is not known.

# Approach
1. Find the three files and the code that stamps them (`retrodoc-render`).
2. Candidates: keep the previous timestamp when the rest of the content is the same; or stamp only when
   something else changed.

# Resources
* `crates/retrodoc-render/`, `docs/crates/render-mcp-cli.md` (idempotence)
* `.claude/skills/smoke-test/SKILL.md` (criterion 3)
