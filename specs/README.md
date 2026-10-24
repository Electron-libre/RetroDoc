# Specifications

One file per capability: what it must do, stated so that a test (or a benchmark) can prove it. A change of
behaviour starts here: write or amend the spec, get it validated, then write the tests from its requirements.

The spec says **what is expected**; an ADR (`docs/adr/`) says **why a choice was made**; `docs/crates/` says
**how the code is laid out**. Specs link to the ADRs that justify them and never repeat their reasoning.

## Format

A spec is `specs/<capability>.md`, in English, naming no confidential test repository. It starts with a
front matter, then these sections, in this order (copy `TEMPLATE.md`):

```markdown
---
id: RER            # three capital letters, unique, prefix of the requirement ids
status: draft      # draft | accepted | implemented
adrs: [0005]       # ADR numbers that justify the spec
---

# Title

## Intent
## Requirements
## Scenarios
## Non-goals
```

### Requirements

One list item per requirement (every top-level item under `## Requirements` is one), `- **<ID>-<n>** (<kind>) <statement>`. The statement says "must" or "must
not". The ids are stable once the spec is `accepted`: tests and issues cite them; a dropped requirement keeps
its number (start its statement with `withdrawn:`, which exempts it from the check) and a new one takes the next number.

The kind says what proves the requirement:

* `deterministic`: a test with a fake `LlmProvider` (no network, no real model) proves it. The test carries a
  `// spec: RER-1` comment right above it, attributes and other comments included, and directly above a `fn` (several ids: `// spec: RER-1, RER-2`).
* `quality`: it depends on what a model writes, so only the benchmark can tell. The item gets indented lines
  `benchmark: benchmark/<repo>/reference.yaml`, `metric: <name>` and an optional `threshold: <value>`. The
  check only verifies that the file exists; it does not run the benchmark (cost, and the spread between
  runs is too wide for a blocking threshold).

### Scenarios

Given/When/Then for the edge cases a statement alone leaves open. A scenario names the requirement it
refines (`(RER-2)`); it is not traced on its own.

### Status

* `draft`: being written, or validated by the user while its tests are still to be written. Not enforced.
* `accepted`: validated by the user and every `deterministic` requirement is traced by a test.
* `implemented`: the code does all of it. Set by hand by whoever delivers the last requirement. Enforced like
  `accepted`.

The order is therefore: write the spec (`draft`), get it validated, write the tests with their
`// spec: ID` (a failing test counts), set `accepted` once all are traced, `implemented` once the code passes.

### Check

`just check-specs` (part of `just check`) fails when: the format is wrong, an id is duplicated, a test cites an
unknown id, an `accepted` or `implemented` spec has a `deterministic` requirement no test cites, or a `quality`
requirement names no existing benchmark file and metric. The script is `.claude/specs/check_specs.rs`, tested by
`just test-harness`. `skill:write-spec` writes and validates a spec.
