---
name: write-spec
description: Write a new spec in `specs/` or amend an existing one (numbered requirements, scenarios, non-goals) with the repo's template and validate it with the traceability checker. Use when a change of behaviour needs its spec before the tests and the code, when the user asks to write, add or amend a spec, when a requirement of an existing spec changes, or to write the spec of an existing capability from its code and tests.
---

# Write a spec

A spec is `specs/<capability>.md`: what a capability must do, stated so that a test can prove
it. The format, the requirement kind and the statuses are in `specs/README.md` (single source of truth:
don't restate them here). This skill is the procedure; `harness run check-specs` (`just check-specs`) enforces the
mechanical rules. Don't implement anything here: the tests come after the user validated the spec.

Show the user your results as `.claude/rules/reports-to-the-user.md` says.

## Procedure

1. **Find the spec**: `ls specs` and read the one that covers the capability. Amend it rather than create a
   second one for the same capability. Read the ADRs it lists and the `//!` doc of the code it describes
   (the `architecture` document and `docs/`) so the requirements say what is true, not what is hoped.
2. **Draft** (new spec: copy `specs/TEMPLATE.md`; pick a free three-letter id with `grep -h '^id:' specs/*.md`):
   * Intent in one or two sentences.
   * One requirement per testable obligation, "must" / "must not", each `(deterministic)`: a test that needs
     no network or external service proves it.
   * Scenarios only for the edge cases a statement leaves open; non-goals for what the spec refuses to
     require. Link ADRs by number in the front matter, never repeat their reasoning.
   * Amending: a new requirement takes the next number; a dropped one keeps its number and its statement
     starts with `withdrawn:`; never renumber an `accepted` spec, tests and issues cite the ids. The spec goes
     back to `draft` while it is amended, and to `accepted` once its new requirements are traced.
   * A point you cannot settle from the repo or the issue: write `[NEEDS CLARIFICATION: <question>]` where it
     belongs instead of guessing.
   * Status `draft` while it is being written. English only, no confidential test repository named.
3. **Check the content** before showing it:
   * each requirement is observable from outside (an exit code, an output, a file), not a property of the code;
   * each boundary a requirement draws (a limit, an empty input, an error) has a scenario;
   * the non-goals cover what a reader would assume the capability does.
4. **Validate the format**: run `just check-specs` and read the lines that name the spec you edited (other specs
   may have their own problems). A problem must be fixed. A wording warning on a requirement you wrote or changed
   is fixed, or kept when it is a false positive and said so in the report; one on a requirement you did not touch
   is left as is. A clarification warning is expected: it becomes a question of the next
   step. A `draft` spec is not asked for tests yet.
5. **Validate with the user (STOP)**: show the requirements and ask what is missing or too strong, with one
   question per `[NEEDS CLARIFICATION: ...]` marker; replace each marker by the settled text once answered. The
   spec stays `draft`.
6. **Trace**: write the tests from the requirements; each carries `// spec: ID` placed as `specs/README.md`
   says. Prove a requirement through the binary when that test is cheap and stable; otherwise an internal test is
   the proof, its citation giving the reason (`// spec: ID (internal: <reason>)`): never write a long or fragile
   end-to-end test only to silence the boundary warning. When every requirement is traced, set `accepted`; `just check-specs` then enforces it.

## Bootstrap the spec of an existing capability

A capability that predates its spec has tests no spec states. Its spec is read back from the code and the tests, not
from an intended change; it runs the procedure above with these differences:

1. **Pick**: `just spec-coverage` lists the test files that cite no requirement of an `accepted` spec, largest first;
   the user picks the capability (or names one).
2. **Read** the code of the capability and every test that drives it.
3. **State only what a test already proves**: one requirement per obligation a test checks, worded from the
   behaviour the test asserts. A spec read back from the code states what the code does, bugs included: say so in
   step 5, the user validates it like any spec and decides whether a bug becomes a requirement or an issue.
4. **Untested behaviour is not a requirement**: list it, then file it as a follow-up issue (`skill:create-issue`)
   whose tests will add the requirements.
5. **Validate** (step 5 above), then **trace** by adding the `// spec: ID` to the existing tests, no new test: a test
   under `src` gets its reason, `// spec: ID (internal: <reason>)`. Every requirement is then traced: set `accepted`.

## Rules

* An obligation no test with fakes can prove (it depends on what a model writes) is not a requirement: reword it
  until a test can, or state it as a non-goal; never leave a requirement untraced.
* `accepted` means validated and traced: never set it before the tests carry their `// spec: ID` (a failing
  test counts, the check only looks for the citation).
