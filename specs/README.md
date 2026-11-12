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
### Behaviour
### Contract
## Scenarios
## Non-goals
```

### Requirements

`## Requirements` holds two subsections, in this order:

* `### Behaviour`: a few requirements in the terms of the product, what the user can do and see, so a reader knows what
  the capability does before any format or code.
* `### Contract`: the formats, the codes, the limits and the refusals. A contract requirement may cite a table that
  follows it after a blank line (one row per field, limit or code); the tests of its rows each cite that requirement.

Both are traced and share one numbering. A spec written before this shape takes it at its next amendment: its
requirements move under the two headings, their ids unchanged. `check-specs` warns of a spec with no `### Behaviour`,
never fails on it.

One list item per requirement (every top-level item under `## Requirements`, outside a fenced block or an HTML comment, is one), `- **<ID>-<n>** (deterministic) <statement>`. The statement holds exactly one "must" or "must
not" (two obligations are two requirements) and none of the vague words `should`, `may`, `etc.`, `appropriate`, `fast`; text in a code span or in double quotes is not checked. A statement may wrap onto the lines that follow its item, up to the end of its paragraph as CommonMark reads it (a blank line, an item, a heading, a fenced block, a quote, an HTML block or a thematic break ends it). The ids are stable once the spec is `accepted`: tests and issues cite them; a dropped requirement keeps
its number (start its statement with `withdrawn:`, which exempts it from the check) and a new one takes the next number.

The kind, `deterministic`, says that a test with no network and no external service (fakes, temp dirs, pure
functions) proves the requirement; what only a model's output could show is not a requirement (a requirement of the
former `quality` kind is read untraced, with a warning: reword it for a test or withdraw it). The test carries a
`// spec: RER-1` comment right above it, attributes and other comments included, and directly above a `fn` (several
ids: `// spec: RER-1, RER-2`). A project whose tests are in another language lists their file-name patterns in
`spec_files` of `.claude/cardinal.toml` (`["*.rs"]` when absent, e.g. `["test_*.py", "*_test.go"]`; `*` is the
only wildcard), and the citation
opens with the line-comment marker of the file's extension (`# spec: RER-1` in Python or shell, `-- spec:` in Lua or
SQL, `;; spec:` in a Lisp; any of the four for an extension `check-specs` does not know), right above the test: past
comments and decorators (`@`), the next line must not be blank. A test under `.claude/` (a managed one) may cite a spec the project does not have: the repository that ships it traces it. Only a prefix with no spec in the project is exempt: an unknown id of a spec the project has, and any citation outside `.claude/`, are still flagged. The repository that ships those tests runs the script with `--strict`, which removes the exemption.

A spec and its tests must survive a rewrite of the code, so a requirement is proven by a test that drives the
program from its boundary (command line, files, exit code, output), placed outside every `src` directory (`tests/`,
the managed tests under `.claude/`). That test is preferred when it is cheap and stable, not required: when a test
through the binary would be impossible, slow or fragile, a test under `src` (one that calls the code from inside,
ideally at the public boundary of a module) is the proof, and its citation gives the reason:
`// spec: RER-1 (internal: <reason>)`, the reason covering every id of the line. `check-specs` warns, per spec, of the
requirements only tests under `src` cite as long as one of those citations at least gives no reason, and fails on a
blank reason. A stack that keeps its internal tests elsewhere names those directories in `internal_dirs` of
`.claude/cardinal.toml` (`["src"]` when absent), which then replace `src`.

### Open questions

A point the writer cannot settle from the repo or the issue is marked where it belongs, never guessed:
`[NEEDS CLARIFICATION: <question>]`, on one line. A `draft` spec may hold markers; an `accepted` one may not. A marker
in a code span or a code block is an example, not a question.

### Scenarios

The section opens with one to three **happy paths**: the main flows of the capability, when all goes well, written
in the terms of its user, so a reader sees what it does before reading its requirements one by one. A happy path
has an id, `<ID>-H<n>`, and names the requirements it walks through:

```markdown
- **RER-H1** (happy; RER-1, RER-4) Given a saved result, when a pass runs, then it reuses it.
```

A test cites it like a requirement (`// spec: RER-H1`), the test that drives the whole flow (the end-to-end test of
the project when there is one); an `accepted` spec needs that citation.

Given/When/Then for the edge cases a statement alone leaves open follow the happy paths. Such a scenario names the
requirement it refines (`(RER-2)`); it is not traced on its own.

### Status

* `draft`: being written, or validated by the user while its tests are still to be written. Not enforced.
* `accepted`: validated by the user and every requirement is traced by a test. A spec still marked `implemented`
  (a former status) is read as `accepted`, with a warning.

The order is therefore: write the spec (`draft`), get it validated, write the tests with their
`// spec: ID` (a failing test counts), set `accepted` once all are traced.

### Check

`cardinal run check-specs` (part of `cardinal run project-check`) fails when: the format is wrong (a happy path
included: its form, the requirements of the spec it names, its place before the other scenarios), an id is
duplicated, a test cites an unknown id, an `accepted` spec has a requirement or a happy path no test cites, or an
`accepted` spec still holds a `[NEEDS CLARIFICATION: ...]` marker. It warns of a spec with no happy path, and of one
with no `### Behaviour` under `## Requirements`, or one that outgrows its capability: over 40 live requirements, 25000
bytes or an Intent of 600 characters (`spec_max_requirements`, `spec_max_bytes`, `spec_max_intent` of
`.claude/cardinal.toml`; it fails on one that is not a positive integer): each `draft` one, and the `accepted` ones in
one line. On a `draft` it only warns (stderr): each marker with its question, and each
requirement with a vague word or not exactly one "must". On any spec it warns of the requirements proven only by tests under `src` with no reason given, and fails on a citation whose reason is blank. `cardinal run check-specs --strict` (for the repository that ships managed tests; consumers stay lenient) also flags the citations of managed tests whose spec the project lacks. The checker is `cardinal run check-specs`, tested by
the tests of the Cardinal source. `skill:write-spec` writes and validates a spec.

`cardinal run spec-coverage` lists the test files that cite no requirement of an `accepted` spec, largest first: the
capabilities worth a spec read back from their code and tests (`skill:write-spec`, "Bootstrap the spec of an existing
capability"). A report, it fails only when git cannot list the files; a `.rs` file is a test file when it holds a test
attribute, a file of another language when `spec_files` matches it. Both read the test files of the whole root,
whatever the layout of the project (`crates/`, or `src/` and `tests/` at the root), but those under `.git`,
`node_modules`, a `target` beside a `Cargo.toml` and the `skipped_dirs` of `.claude/cardinal.toml`.
