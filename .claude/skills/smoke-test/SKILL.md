---
name: smoke-test
description: Run RetroDoc end to end on a real repository with the local Ollama LLM and judge whether the run is conclusive (generate finishes, no warning, rerun is a no-op that calls no LLM, report produced). Use when asked for a smoke test, to validate a pipeline change on a real repo, or to reproduce a field failure.
---

# Smoke test on a real repository

Unit tests use a fake `LlmProvider`; this checks the real thing. The repository under test is a
**parameter**: ask the user for the path if they didn't give one.

## Confidentiality (hard rule)

Target repositories may belong to a client. **Never write their name, path, file names or code excerpts**
in anything committed: docs, `PLAN.md`, `issues/`, ADRs, commit messages, tests. Describe them generically
("a large Rails repository", "a small Rust crate") and give figures (files, duration, features) only.
Logs and the clone stay in `/tmp`.

## Run

```sh
just smoke <path-to-target-repo>              # defaults below
just smoke <repo> --model <m> --base-url <u>  # another model or server
```

It takes minutes to hours, so launch it with `run_in_background` and read the logs
(`/tmp/retrodoc-smoke-<repo-dir-name>.run1.log`, `.run2.log`, `.report.txt`) while it runs: progress lines
show rank, percentage and ETA, and a "still waiting for the LLM (Ns)" line every 30 s means a slow call,
not a stuck run.

What `smoke.rs run` does: refuses to start if no server answers; clones the committed `HEAD` of the target
into `/tmp` (the original is never touched, uncommitted changes are not tested); `retrodoc init` then points
`[llm]` at the local server; runs `generate`, `generate` again, then `report`; prints the verdict.

## Local LLM defaults

- Ollama instance on `http://localhost:11435/v1/chat/completions` (the `ollama-amd` systemd service, meant
  for the Radeon iGPU); the other one on `:11434` is the busy NVIDIA card. Don't start another instance.
- Model `qwen3.6:35b-a3b` with `reasoning_effort = "none"` (it is a "thinking" model, far too slow
  otherwise). A dummy `OPENROUTER_API_KEY` is enough; `smoke.rs` sets it.
- The server context must be large (`OLLAMA_CONTEXT_LENGTH=32768`, set server-side; the default 4096
  silently truncates prompts and cuts JSON answers). Check with `OLLAMA_HOST=127.0.0.1:11435 ollama ps`
  (column CONTEXT). A `finish_reason=length` warning in the log is the symptom.
- Don't run two smoke tests against the same Ollama at once.

## Conclusive means

1. `generate` ends with "N file(s) written" (reached the render step) and exits 0;
2. no `WARN` line in either run (an unparseable unit that was skipped shows up as a warning);
3. the second `generate` prints "0 file(s) written" (caches and render are idempotent);
4. the second `generate` made no LLM call: its recap says `LLM usage: no call` (a run without any call is not
   saved in `usage.json`, so the recap is the proof; a missing recap fails too). When it did call, the last
   `generate` entry of `.retrodoc/cache/usage.json` names the passes. This criterion is blocking;
5. `report` exits 0 and is not empty.

`just smoke` prints `SMOKE TEST CONCLUSIVE` or one `FAIL - …` per broken criterion. To judge logs from a
manual run: `rust-script .claude/skills/smoke-test/smoke.rs evaluate <run1> <run2> <report> <usage.json>`.

## After the run

- Report the verdict, the duration and the figures (files, domains, features, use cases, confidence).
- A FAIL is a finding, not noise: read the log around the first `WARN`/error, say what it is and whether it
  comes from the pipeline or the target repository (a repo that breaks its own layering is its flaw, not a
  RetroDoc bug). Don't paper over it by editing the generated rules by hand.
- Don't commit anything produced by the run; the clone and logs live in `/tmp`.
