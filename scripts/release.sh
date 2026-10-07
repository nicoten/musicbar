#!/usr/bin/env bash
# Builds a signed + notarized universal MusicBar, then publishes it as a GitHub release with the
# updater manifest (latest.json) that installed copies poll.
#   scripts/release.sh 0.2.0          bump version, build, commit, tag, push, publish
#   scripts/release.sh 0.2.0 --local  just bump and build (nothing leaves this machine)
#
# Needs APPLE_SIGNING_IDENTITY / APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID for signing and
# notarization, and the updater key at ~/.tauri/musicbar.key. The key is passed explicitly so a
# TAURI_SIGNING_PRIVATE_KEY exported for another app never signs MusicBar's updates.
set -euo pipefail

REPO="nicoten/musicbar"
KEY="$HOME/.tauri/musicbar.key"
TARGET="universal-apple-darwin"

version="${1:-}"
local_only="${2:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: scripts/release.sh <major.minor.patch> [--local]" >&2
  exit 1
fi
cd "$(dirname "$0")/.."

if [[ -z "$local_only" ]]; then
  if [[ -n "$(git status --porcelain)" ]]; then
    echo "working tree is not clean" >&2
    exit 1
  fi
  if gh release view "v$version" --repo "$REPO" >/dev/null 2>&1; then
    echo "v$version is already released" >&2
    exit 1
  fi
fi

# The updater compares against tauri.conf.json's version, so all three must agree.
sed -i '' "s/^  \"version\": \"[0-9.]*\"/  \"version\": \"$version\"/" package.json src-tauri/tauri.conf.json
# Only the [package] version (the first one), not any dependency's.
perl -0pi -e "s/^version = \"[0-9.]*\"/version = \"$version\"/m" src-tauri/Cargo.toml
npm install --package-lock-only --silent
(cd src-tauri && cargo update --workspace --quiet)

env -u TAURI_SIGNING_PRIVATE_KEY_PATH \
  TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY")" \
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
  npx tauri build --target "$TARGET"

bundle="src-tauri/target/$TARGET/release/bundle"
tarball="$bundle/macos/MusicBar.app.tar.gz"
dmg="$bundle/dmg/MusicBar_${version}_universal.dmg"
manifest="$bundle/latest.json"
url="https://github.com/$REPO/releases/download/v$version/MusicBar.app.tar.gz"

# One universal build serves both Apple silicon and Intel.
python3 - "$version" "$url" "$tarball.sig" "$manifest" <<'PY'
import json, sys, datetime
version, url, sig_path, out = sys.argv[1:]
entry = {"signature": open(sig_path).read().strip(), "url": url}
json.dump({
    "version": version,
    "notes": f"MusicBar v{version}",
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {"darwin-aarch64": entry, "darwin-x86_64": entry},
}, open(out, "w"), indent=2)
PY

if [[ -n "$local_only" ]]; then
  echo "Built v$version locally:"
  ls -1 "$tarball" "$dmg" "$manifest"
  exit 0
fi

git add package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git diff --cached --quiet || git commit -q -m "Release v$version"
git tag -a "v$version" -m "MusicBar v$version"
git push -q
git push -q origin "v$version"
gh release create "v$version" "$dmg" "$tarball" "$manifest" \
  --repo "$REPO" --title "MusicBar v$version" --notes "MusicBar v$version"
echo "Released v$version: https://github.com/$REPO/releases/tag/v$version"
