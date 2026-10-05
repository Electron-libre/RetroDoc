# 0013. Chunk long files with LLM-proposed boundaries, verified mechanically

Status: Accepted

_Retroactive ADR, reconstructed from the history (f232c74, 1a1b5a4, ea32938, c4f2847, c22388a; 2026-10-03)._

## Context

Large controllers and model files were truncated or made the call time out and overflow the output
limit (0012): a 34 KB controller with 65 actions yielded 9 entry points. Where to cut depends on the
language, and the walker knows few languages.

## Decision

Long files are read in chunks (about 5,000 characters for entry points, 4,000 for models), sent as
"(part i/n)", with at most 8 chunks per file; results are saved once all chunks answered and merged by
name. Cut points come from the LLM instead of hard-coded per-language rules: the roles call also returns
`chunk_boundaries` (extensions + a regex matching the line that starts a module/class/function), saved
in `roles.yaml`. The chunker cuts before the last such line in the second half of a chunk, else after a
blank line, else anywhere. Since a proposed regex is untrusted, `chunk_check.rs` measures it on the 20
largest files of the extension with a language-agnostic probe, covering definition lines and the
decorators above: under 90% the LLM gets up to 3 focused fix calls (compiler errors are reported back,
prompts spell out the regex dialect), under 50% the rule is dropped for the blank-line fallback, and a
regex matching over 40% of lines is rejected as too broad. The use cases and confidence passes show
`Splitter::excerpt` (first chunk plus the chunks matching the entry point names, feature words or cited
lines, with omitted ranges marked) instead of the file head.

Hard-coded per-language cutters were rejected (maintenance, unknown languages); trusting the LLM's
regex was rejected after a js/ts regex ending in `|` matched every line at "100%".

## Consequences

Entry points on the test controller went from 9 to 37 and from 469 to 810 over the whole repo. Chunking
costs more calls and the cut quality is measured, not guaranteed. The actors pass and repo map still
truncate late code.
