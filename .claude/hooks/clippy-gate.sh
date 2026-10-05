#!/usr/bin/env bash
# Stop hook: if Rust files changed, clippy must be warning-free before the agent can finish.
# Exit 2 sends clippy's output back to the agent so it fixes the code. When the agent
# already retried once (stop_hook_active), let go to avoid an endless loop.
set -u
input="$(cat)"
[ "$(echo "$input" | jq -r '.stop_hook_active // false')" = "true" ] && exit 0
cd "${CLAUDE_PROJECT_DIR:-$PWD}" || exit 0
git status --porcelain 2>/dev/null | grep -q '\.rs$' || exit 0
if out="$(cargo clippy --workspace --all-targets -- -D warnings 2>&1)"; then
  exit 0
fi
{ echo "cargo clippy reports problems; fix them (the project must stay warning-free):"; echo "$out"; } >&2
exit 2
