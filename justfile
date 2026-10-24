# Development commands. `just` lists them; the agent harness uses the same entry points.
import 'harness.just'

default:
    @just --list

# Format, lint (warning-free) and test the whole workspace.
check: fmt clippy test check-specs check-harness

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

# One HTTP call: does a server reject, ignore or obey a strict JSON schema? (ollama|deepseek|gemini|openrouter)
probe-schema *args:
    cargo run -q -p retrodoc-llm --example schema_probe -- {{args}}

# Smoke test on a real repo with the local LLM (see .claude/skills/smoke-test/SKILL.md). Slow.
smoke *args:
    rust-script .claude/skills/smoke-test/smoke.rs run {{args}}

# Quality benchmark on a public repo against its hand-written reference (see .claude/skills/quality-benchmark/SKILL.md). Slow.
benchmark *args:
    rust-script .claude/skills/quality-benchmark/benchmark.rs run {{args}}
