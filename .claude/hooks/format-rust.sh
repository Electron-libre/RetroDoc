#!/usr/bin/env bash
# PostToolUse hook (Edit|Write): run `cargo fmt --all` after a Rust file was edited.
set -u
file="$(jq -r '.tool_input.file_path // empty')"
case "$file" in
  *.rs) ;;
  *) exit 0 ;;
esac
cd "${CLAUDE_PROJECT_DIR:-$PWD}" || exit 0
cargo fmt --all 2>&1 >&2 || true
exit 0
