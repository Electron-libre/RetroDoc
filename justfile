# Development commands. `just` lists them; the agent harness uses the same entry points.
import 'cardinal.just'

default:
    @just --list

# Format, lint (warning-free), test the whole workspace and check the specs (`check_command`, then the specs).
check:
    cardinal run project-check

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings
    # The workspace turns `bench` on (ADR 0042): the default build of the pipeline is linted alone.
    cargo clippy -p retrodoc-pipeline --lib -- -D warnings

test:
    cargo test --workspace

# Set the version of the workspace and of its crates in the lock file (`release_version_command` of `release`).
set-version version:
    sed -i 's/^version = "[0-9.]*"$/version = "{{version}}"/' Cargo.toml
    cargo update --workspace --offline

# Test the scripts of the project's own skills (smoke-test, quality-benchmark) and of `scripts/`. Requires `rust-script`. Prints
# nothing on success, and only the lines of the failing script that are not `ok` otherwise.
test-skill-scripts:
    #!/usr/bin/env sh
    for t in .claude/skills/*/test_*.rs scripts/test_*.rs; do
        [ -e "$t" ] || continue
        out=$(rust-script "$t" 2>&1) || { printf '%s\n' "$out" | grep -v '^ok '; echo "FAILED: $t"; exit 1; }
    done

# One HTTP call: does a server reject, ignore or obey a strict JSON schema? (ollama|deepseek|gemini|openrouter)
probe-schema *args:
    cargo run -q -p retrodoc-llm --example schema_probe -- {{args}}

# Screenshots of every page of a running `retrodoc ui` at `<base-url>`, into `<runs>/screenshots/<label>` (`docs/dev/web-ui.md`).
screenshots base-url label:
    rust-script scripts/screenshots.rs shoot {{base-url}} {{label}}

# Smoke test on a real repo with the local LLM (see .claude/skills/smoke-test/SKILL.md). Slow.
smoke *args:
    rust-script .claude/skills/smoke-test/smoke.rs run {{args}}

# Quality benchmark on a public repo against its hand-written reference (see .claude/skills/quality-benchmark/SKILL.md). Slow.
benchmark *args:
    rust-script .claude/skills/quality-benchmark/benchmark.rs run {{args}}
