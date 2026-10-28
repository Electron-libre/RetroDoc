#!/usr/bin/env sh
# Put on the CI path the `harness` of the revision `.claude/harness.toml` pins, without Rust when the source publishes
# it (ADR 0010): the release asset of the commit, checked against its SHA-256, goes where `harness run` looks for it
# (`~/.cache/harness/bin/<commit>/<target>`). Otherwise `cargo install`: from the git source at `rev` (into that same
# place when `rev` is a commit, so `harness run` does not build it again), or from the crate of a local source in this
# repository. Run from the top of the work tree by the jobs of `harness.yml`.
set -eu

value() { sed -n "s/^$1 *= *\"\([^\"]*\)\".*/\1/p" .claude/harness.toml | head -n 1; }
source=$(value source)
rev=$(value rev)

case "$source" in
  *://*|git@*) ;;
  *)
    if [ -f "$source/crates/harness/Cargo.toml" ]; then
      exec cargo install --quiet --path "$source/crates/harness" --locked
    fi
    echo "source '$source' is a local path outside this repository: harness not installed"
    exit 0 ;;
esac

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-musl ;;
  Linux-aarch64) target=aarch64-unknown-linux-musl ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  *) target=unknown ;;
esac
# The releases are named after the full commit hash, which `harness update` writes to `rev`.
case "$rev" in
  *[!0-9a-f]*) full="" ;;
  *) if [ ${#rev} -eq 40 ] || [ ${#rev} -eq 64 ]; then full=$rev; else full=""; fi ;;
esac
[ -n "$full" ] || echo "rev '$rev' is not a full commit hash (run \`harness update\` to pin one): no release to fetch"
case "$source" in
  https://github.com/*|git@github.com:*|ssh://git@github.com/*) on_github=yes ;;
  *) on_github="" ;;
esac
repo=$(printf '%s' "$source" | sed -E 's#^(https://github\.com/|git@github\.com:|ssh://git@github\.com/)##; s#/$##; s#\.git$##')
# cargo reads no scp-style `git@host:path`
cargo_url=$(printf '%s' "$source" | sed -E '/:\/\//!s#^([^/:]+@[^:]+):/?#ssh://\1/#')
dir="${XDG_CACHE_HOME:-$HOME/.cache}/harness/bin/$full/$target"
url="https://github.com/$repo/releases/download/harness-$full/harness-$target"

if [ -n "$full" ] && [ -n "$on_github" ] && mkdir -p "$dir" \
  && curl -fsSL -o "$dir/.partial" "$url" && curl -fsSL -o "$dir/.sha256" "$url.sha256"; then
  digest=$(cut -d ' ' -f 1 "$dir/.sha256")
  actual=$( (sha256sum "$dir/.partial" 2>/dev/null || shasum -a 256 "$dir/.partial") | cut -d ' ' -f 1)
  rm -f "$dir/.sha256"
  if [ "$digest" != "$actual" ]; then
    rm -f "$dir/.partial"
    echo "harness $rev for $target does not match its published SHA-256: not installed" >&2
    exit 1
  fi
  chmod +x "$dir/.partial"
  mv "$dir/.partial" "$dir/harness"
  echo "$dir" >> "$GITHUB_PATH"
  echo "harness $rev for $target: release asset"
elif [ -n "$full" ]; then
  rm -f "$dir/.partial" "$dir/.sha256"
  echo "harness $rev: no release asset, building it"
  cargo install --quiet --git "$cargo_url" --rev "$full" --locked --root "$dir/.root" harness
  mv "$dir/.root/bin/harness" "$dir/harness"
  rm -rf "$dir/.root"
  echo "$dir" >> "$GITHUB_PATH"
else
  cargo install --quiet --git "$cargo_url" ${rev:+--rev "$rev"} --locked harness
fi
