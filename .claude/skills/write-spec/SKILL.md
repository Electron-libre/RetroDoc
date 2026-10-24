---
name: write-spec
description: Write a new spec in `specs/` or amend an existing one (numbered requirements, scenarios, non-goals) with the repo's template and validate it with the traceability checker. Use when a change of behaviour needs its spec before the tests and the code, when the user asks to write, add or amend a spec, or when a requirement of an existing spec changes.
---

# Write a spec

A spec is `specs/<capability>.md`: what a capability must do, stated so that a test or a benchmark can prove
it. The format, the requirement kinds and the statuses are in `specs/README.md` (single source of truth:
don't restate them here). This skill is the procedure; `.claude/specs/check_specs.rs` enforces the
mechanical rules. Don't implement anything here: the tests come after the user validated the spec.

## Procedure

1. **Find the spec**: `ls specs` and read the one that covers the capability. Amend it rather than create a
   second one for the same capability. Read the ADRs it lists and the `//!` doc of the code it describes
   (`docs/crates/`) so the requirements say what is true, not what is hoped.
2. **Draft** (new spec: copy `specs/TEMPLATE.md`; pick a free three-letter id with `grep -h '^id:' specs/*.md`):
   * Intent in one or two sentences.
   * One requirement per testable obligation, "must" / "must not", the kind of each (`deterministic` if a
     test with a fake `LlmProvider` can prove it, `quality` if only a model's output can: then add its
     `benchmark:` and `metric:` lines).
   * Scenarios only for the edge cases a statement leaves open; non-goals for what the spec refuses to
     require. Link ADRs by number in the front matter, never repeat their reasoning.
   * Amending: a new requirement takes the next number; a dropped one keeps its number and its statement
     starts with `withdrawn:`; never renumber an `accepted` spec, tests and issues cite the ids.
   * Status `draft` while it is being written. English only, no confidential test repository named.
3. **Validate the format**: run `just check-specs` and read the lines that name the spec you edited; there
   must be none (other specs may have their own problems). A `draft` spec is not asked for tests yet.
4. **Validate with the user (STOP)**: show the requirements and ask what is missing or too strong. The
   spec stays `draft`.
5. **Trace**: write the tests from the requirements; each carries `// spec: ID` placed as `specs/README.md`
   says. When every `deterministic` requirement is traced, set `accepted`; `just check-specs` then enforces
   it. Set `implemented` when the code does all of it.

## Rules

* A requirement that can't be proved by a test with a fake provider is `quality`, never left untraced.
* `accepted` means validated and traced: never set it before the tests carry their `// spec: ID` (a failing
  test counts, the check only looks for the citation).
