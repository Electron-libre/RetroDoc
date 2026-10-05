#!/usr/bin/env bash
# Behavior test for the Rust hooks: runs them against a throwaway Cargo project.
# Usage: .claude/hooks/test-hooks.sh
set -u
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export CLAUDE_PROJECT_DIR="$tmp"
fail=0
check() { # check <description> <expected-exit> <actual-exit>
  if [ "$2" = "$3" ]; then echo "ok   - $1"; else echo "FAIL - $1 (expected exit $2, got $3)"; fail=1; fi
}

cd "$tmp"
git init -q . && cargo init -q --name hooktest . 2>/dev/null
git add -A && git -c user.email=t@t -c user.name=t commit -qm init

# format hook: reformats an edited .rs file, ignores other files
printf 'fn main(){let  a=1;println!("{a}");}\n' > src/main.rs
echo "{\"tool_input\":{\"file_path\":\"$tmp/src/main.rs\"}}" | "$here/format-rust.sh" >/dev/null 2>&1
check "format hook exits 0" 0 $?
grep -q 'let a = 1;' src/main.rs; check "format hook reformatted the file" 0 $?
echo "{\"tool_input\":{\"file_path\":\"$tmp/README.md\"}}" | "$here/format-rust.sh" >/dev/null 2>&1
check "format hook ignores non-Rust files" 0 $?

# clippy gate: no Rust change -> pass
git add -A && git -c user.email=t@t -c user.name=t commit -qm fmt
echo '{}' | "$here/clippy-gate.sh" >/dev/null 2>&1
check "gate passes with no Rust change" 0 $?

# clippy gate: warning -> exit 2 with the output on stderr
printf 'fn main() {\n    let unused = 1;\n}\n' > src/main.rs
out="$(echo '{}' | "$here/clippy-gate.sh" 2>&1 >/dev/null)"; code=$?
check "gate blocks on a clippy warning" 2 $code
echo "$out" | grep -q 'unused'; check "gate reports the warning to the agent" 0 $?

# clippy gate: no infinite loop when the agent already retried
echo '{"stop_hook_active":true}' | "$here/clippy-gate.sh" >/dev/null 2>&1
check "gate lets go when stop_hook_active" 0 $?

# clippy gate: fixed code -> pass
printf 'fn main() {}\n' > src/main.rs
echo '{}' | "$here/clippy-gate.sh" >/dev/null 2>&1
check "gate passes once fixed" 0 $?

exit $fail
