# Share the skeleton of the LLM passes and the context of a run

# Goal
Each pass writes again the same sequence: load the saved artifact, fingerprint the input of a unit, skip
the unchanged ones, call the LLM, ground the answer, save, report progress. The orchestration threads the
same five or six values through every function. Extract what is common so a pass reads as its prompt, its
answer type and its grounding, and a fix of the skeleton (resume, concurrency, fingerprint) is made once.
No behaviour change: the tests, `just check` and a second `generate` writing nothing stay green.

# Findings (project review of 2026-10-12, from the structure; the modules were not all read line by line)
* Twelve `build_*` entry points in `retrodoc-pipeline` with different signatures; 21 `load`/`save`
  functions outside the tests.
* `batched_read.rs` already shares the loop of the glossary and entry points passes
  (`issues/done/refactor_code_hygiene.md`, deliverable 1); the per-unit passes (features, use cases, rules,
  confidence) each have their own loop, with concurrency added to two of them one after the other.
* `business_read/`, `glossary/` and `entry_points/` read the same files with three prompts; since ADR 0028
  the joint reading is the default and the two others read only what it left.
* `generate/mod.rs` is 648 lines; `features_from_surface` and `features_from_files` take 8 arguments
  (`clippy::too_many_arguments` allowed twice); `repo_root`, `brief`, `evidence`, `llm` and `tracker` go
  through almost every function; `UseCaseContext` is rebuilt to change one field for the rules.
* Largest files: `retrodoc-mcp/src/tools.rs` 1,059 lines, `retrodoc-llm/src/openrouter.rs` 990,
  `retrodoc-render/src/markdown.rs` 923, `retrodoc-pipeline/src/report.rs` 744, `chunks.rs` 706.
* `chunk_check.rs` (486 lines) verifies the boundary regexes the LLM proposes; since ADR 0027 the five
  parsed languages no longer use it.

# Approach
One deliverable per point, each a `refactor(...)` commit:
1. Read the per-unit passes side by side and write down what really is the same (it may be less than it
   looks); only then choose between a generic runner (trait or function taking the unit, the fingerprint,
   the call and the grounding) and smaller shared helpers.
2. A run context for `generate` (repository root, configuration, providers, tracker, brief, evidence)
   instead of long argument lists.
3. The reading passes: one module for the joint reading and its two single-purpose variants, if the
   standalone commands `glossary` and `entry-points` are kept; else remove them (a decision for the user).
4. The regex boundaries: measure which extensions of the smoke and benchmark repositories still reach
   `chunk_check`; keep, or fall back on blank lines for unparsed languages and remove the calls.
5. Split the largest files along what they already hold (tests apart, one tool per file).

# Resources
* `crates/retrodoc-pipeline/src/batched_read.rs`, `features/`, `use_cases/`, `rules/`, `confidence/`
* `crates/retrodoc-cli/src/commands/generate/mod.rs`, `commands/usage.rs`, `commands/workspace.rs`
* `issues/done/refactor_code_hygiene.md` (what was already done, and how)
* ADRs 0003, 0005, 0013, 0027 and 0028

# Hints
* After `single_pipeline_path`: do not refactor code about to be removed.
* An abstraction that hides the differences between passes behind many parameters is worse than the
  duplication: stop where the passes really differ.
* The fingerprints must not change, or every user's next run redoes everything.
