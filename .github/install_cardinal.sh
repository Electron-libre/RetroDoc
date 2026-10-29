#!/usr/bin/env sh
# Put on the CI path the `cardinal` of the revision `.claude/cardinal.toml` pins, without Rust when the source
# publishes it (ADR 0010): the release asset of the commit, checked against its SHA-256, goes where `cardinal run`
# looks for it (`~/.cache/cardinal/bin/<commit>/<target>`). Otherwise `cargo install`: from the git
# source at `rev` (into that same place when `rev` is a commit, so `cardinal run` does not build it again), or from the
# crate of a local source in this repository. Run from the top of the work tree by the jobs of `cardinal.yml`.
set -eu

config=.claude/cardinal.toml
[ -f "$config" ] || { echo "no $config: cardinal not installed" >&2; exit 1; }
# a basic or a literal TOML string, the key indented or not
value() {
  key="^[[:space:]]*$1[[:space:]]*=[[:space:]]*"
  sed -n -e "s/$key\"\([^\"]*\)\".*/\1/p" -e "s/$key'\([^']*\)'.*/\1/p" "$config" | head -n 1
}
source=$(value source)
# no key, another form of string or an unreadable file: the local-path branch below would take '' for a path
[ -n "$source" ] || { echo "no source read from $config: cardinal not installed" >&2; exit 1; }
rev=$(value rev)

# A path git source as the absolute `file://` URL of that path from the top of the work tree, its symlinks resolved, as
# `cardinal` reads it (SYN-50): `.` and `..` removed, every byte outside `A-Za-z0-9-._~/` percent-encoded.
file_url() {
  case "$1" in /*) path=$1 ;; *) path=$(pwd -P)/$1 ;; esac
  out=""
  set -f; ifs=$IFS; IFS=/
  for part in $path; do
    case "$part" in
      ''|.) ;;
      ..) out=${out%/*} ;;
      *) out="$out/$part" ;;
    esac
  done
  IFS=$ifs; set +f
  printf 'file://'
  printf '%s' "${out:-/}" | od -An -v -tx1 | tr -s ' ' '\n' | while read -r byte; do
    case "$byte" in
      '') ;;
      2d|2e|2f|3[0-9]|4[1-9a-f]|5[0-9a]|5f|6[1-9a-f]|7[0-9a]|7e) printf "\\$(printf '%03o' "0x$byte")" ;;
      *) printf '%%%s' "$(printf '%s' "$byte" | tr a-f A-F)" ;;
    esac
  done
}

# git as `cardinal` reads it (SYN-49): a URL, `git@`, or ending with `.git`; a path one resolved (SYN-50), a scp-like
# `host:path` kept
case "$source" in
  *://*) ;;
  git@*|*.git) case "${source%%/*}" in *:*) ;; *) source=$(file_url "$source") ;; esac ;;
  *)
    if [ -f "$source/crates/cardinal/Cargo.toml" ]; then
      exec cargo install --quiet --force --path "$source/crates/cardinal" --locked
    fi
    echo "source '$source' is a local path outside this repository: cardinal not installed"
    exit 0 ;;
esac

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-musl ;;
  Linux-aarch64) target=aarch64-unknown-linux-musl ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  *) target=unknown ;;
esac
# The releases are named after the full commit hash, which `cardinal update` writes to `rev`.
case "$rev" in
  *[!0-9a-f]*) full="" ;;
  *) if [ ${#rev} -eq 40 ] || [ ${#rev} -eq 64 ]; then full=$rev; else full=""; fi ;;
esac
[ -n "$full" ] || echo "rev '$rev' is not a full commit hash (run \`cardinal update\` to pin one): no release to fetch"
case "$source" in
  https://github.com/*|git@github.com:*|ssh://git@github.com/*) on_github=yes ;;
  *) on_github="" ;;
esac
repo=$(printf '%s' "$source" | sed -E 's#^(https://github\.com/|git@github\.com:|ssh://git@github\.com/)##; s#/$##; s#\.git$##')
# cargo reads no scp-style `[user@]host:path`
cargo_url=$(printf '%s' "$source" | sed -E '/:\/\//!s#^([^/:]+):/?#ssh://\1/#')
dir="${XDG_CACHE_HOME:-$HOME/.cache}/cardinal/bin/$full/$target"
releases="https://github.com/$repo/releases/download"

# The release of the commit, else its release under the name before the rename.
fetched() {
  curl -fsSL -o "$dir/.partial" "$releases/$1-$full/$1-$target" \
    && curl -fsSL -o "$dir/.sha256" "$releases/$1-$full/$1-$target.sha256"
}

if [ -n "$full" ] && [ -n "$on_github" ] && mkdir -p "$dir" && { fetched cardinal || fetched harness; }; then
  digest=$(cut -d ' ' -f 1 "$dir/.sha256")
  actual=$( (sha256sum "$dir/.partial" 2>/dev/null || shasum -a 256 "$dir/.partial") | cut -d ' ' -f 1)
  rm -f "$dir/.sha256"
  if [ "$digest" != "$actual" ]; then
    rm -f "$dir/.partial"
    echo "cardinal $rev for $target does not match its published SHA-256: not installed" >&2
    exit 1
  fi
  chmod +x "$dir/.partial"
  mv "$dir/.partial" "$dir/cardinal"
  echo "$dir" >> "$GITHUB_PATH"
  echo "cardinal $rev for $target: release asset"
elif [ -n "$full" ]; then
  rm -f "$dir/.partial" "$dir/.sha256"
  echo "cardinal $rev: no release asset, building it"
  # no crate named: the single package of the commit
  cargo install --quiet --git "$cargo_url" --rev "$full" --locked --root "$dir/.root"
  # `harness` for a commit from before the rename
  mv "$dir/.root/bin/cardinal" "$dir/cardinal" 2>/dev/null || mv "$dir/.root/bin/harness" "$dir/cardinal"
  rm -rf "$dir/.root"
  echo "$dir" >> "$GITHUB_PATH"
else
  # --force: over the installed `cardinal`, even of the same version
  cargo install --quiet --force --git "$cargo_url" ${rev:+--rev "$rev"} --locked
fi
