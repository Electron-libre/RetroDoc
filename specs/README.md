# Specifications

One file per capability: what it must do, stated so that a test can prove it. A change of
behaviour starts here: write or amend the spec, get it validated, then write the tests from its requirements.

The spec says **what is expected**; an ADR (`docs/adr/`) says **why a choice was made**; `docs/*.md` says
**how the code is laid out**. Specs link to the ADRs that justify them and never repeat their reasoning.

## Format

A spec is `specs/<capability>.md`, in English, naming no confidential test repository. It starts with a
front matter, then these sections, in this order (copy `TEMPLATE.md`):

```markdown
---
id: RER            # three capital letters, unique, prefix of the requirement ids
status: draft      # draft | accepted
adrs: [0005]       # ADR numbers that justify the spec
---

# Title

## Intent
## Requirements
## Scenarios
## Non-goals
```

### Requirements

One list item per requirement (every top-level item under `## Requirements` is one), `- **<ID>-<n>** (deterministic) <statement>`. The statement holds exactly one "must" or "must
not" (two obligations are two requirements) and none of the vague words `should`, `may`, `etc.`, `appropriate`, `fast`; text in a code span or in double quotes is not checked. The ids are stable once the spec is `accepted`: tests and issues cite them; a dropped requirement keeps
its number (start its statement with `withdrawn:`, which exempts it from the check) and a new one takes the next number.

The kind, `deterministic`, says that a test with no network and no external service (fakes, temp dirs, pure
functions) proves the requirement; what only a model's output could show is not a requirement (a requirement of the
former `quality` kind is read untraced, with a warning: reword it for a test or withdraw it). The test carries a
`// spec: RER-1` comment right above it, attributes and other comments included, and directly above a `fn` (several
ids: `// spec: RER-1, RER-2`). A test under `.claude/` (a managed one) may cite a spec the project does not have: the repository that ships it traces it. Only a prefix with no spec in the project is exempt: an unknown id of a spec the project has, and any citation outside `.claude/`, are still flagged. The repository that ships those tests runs the script with `--strict`, which removes the exemption.

A spec and its tests must survive a rewrite of the code, so a requirement is proven by a test that drives the
program from its boundary (command line, files, exit code, output), placed outside every `src` directory (`tests/`,
the managed tests under `.claude/`). That test is preferred when it is cheap and stable, not required: when a test
through the binary would be impossible, slow or fragile, a test under `src` (one that calls the code from inside,
ideally at the public boundary of a module) is the proof, and its citation gives the reason:
`// spec: RER-1 (internal: <reason>)`, the reason covering every id of the line. `check-specs` warns, per spec, of the
requirements only tests under `src` cite as long as one of those citations at least gives no reason, and fails on a
blank reason.

### Open questions

A point the writer cannot settle from the repo or the issue is marked where it belongs, never guessed:
`[NEEDS CLARIFICATION: <question>]`, on one line. A `draft` spec may hold markers; an `accepted` one may not. A marker
in a code span or a fenced code block is an example, not a question.

### Scenarios

Given/When/Then for the edge cases a statement alone leaves open. A scenario names the requirement it
refines (`(RER-2)`); it is not traced on its own.

### Status

* `draft`: being written, or validated by the user while its tests are still to be written. Not enforced.
* `accepted`: validated by the user and every requirement is traced by a test. A spec still marked `implemented`
  (a former status) is read as `accepted`, with a warning.

The order is therefore: write the spec (`draft`), get it validated, write the tests with their
`// spec: ID` (a failing test counts), set `accepted` once all are traced.

### Check

`just check-specs` (part of `just check`) fails when: the format is wrong, an id is duplicated, a test cites an
unknown id, an `accepted` spec has a requirement no test cites, or an `accepted` spec still holds a
`[NEEDS CLARIFICATION: ...]` marker. On a `draft` it only warns (stderr): each marker with its question, and each
requirement with a vague word or not exactly one "must". On any spec it warns of the requirements proven only by tests under `src` with no reason given, and fails on a citation whose reason is blank. `just check-specs --strict` (for the repository that ships managed tests; consumers stay lenient) also flags the citations of managed tests whose spec the project lacks. The checker is `harness run check-specs`, tested by
the tests of the harness source. `skill:write-spec` writes and validates a spec.

`just spec-coverage` lists the test files that cite no requirement of an `accepted` spec, largest first: the
capabilities worth a spec read back from their code and tests (`skill:write-spec`, "Bootstrap the spec of an existing
capability"). A report, it fails only when git cannot list the files. Both read the `.rs` files of the whole root,
whatever the layout of the project (`crates/`, or `src/` and `tests/` at the root), but those under `.git`,
`node_modules`, a `target` beside a `Cargo.toml` and the `skipped_dirs` of `.claude/harness.toml`.
