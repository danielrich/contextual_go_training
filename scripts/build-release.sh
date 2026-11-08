#!/usr/bin/env bash
set -euo pipefail
WB_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$WB_ROOT"
WB_BUILD_TARGET="${CARGO_TARGET_DIR:-$WB_ROOT/target}"
case "$WB_BUILD_TARGET" in /*) ;; *) WB_BUILD_TARGET="$WB_ROOT/$WB_BUILD_TARGET" ;; esac
if [[ "$WB_ROOT" != *' '* && "${HOME:-}" != *' '* ]]; then
  export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$WB_ROOT=/src/whole-board --remap-path-prefix=${HOME:-/nonexistent}/.cargo=/cargo"
fi
cargo build --locked --release --target-dir "$WB_BUILD_TARGET"
WB_PLATFORM="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
WB_OUTPUT="$WB_ROOT/dist/whole-board-$WB_PLATFORM"
mkdir -p "$WB_OUTPUT/bin" "$WB_OUTPUT/static" "$WB_OUTPUT/deploy"
install -m 755 "$WB_BUILD_TARGET/release/whole-board" "$WB_OUTPUT/bin/whole-board"
cp "$WB_ROOT"/static/* "$WB_OUTPUT/static/"
cp "$WB_ROOT"/deploy/* "$WB_OUTPUT/deploy/"
cp "$WB_ROOT/README.md" "$WB_OUTPUT/README.md"
cp "$WB_ROOT/scripts/install-service.sh" "$WB_OUTPUT/install-service.sh"
cp "$WB_ROOT/scripts/verify-deploy.sh" "$WB_OUTPUT/verify-deploy.sh"
WB_REVISION=$(git rev-parse --short HEAD 2>/dev/null || printf 'source')
WB_BUILD_ID="${WB_BUILD_ID:-$WB_REVISION-$(date -u +%Y%m%dT%H%M%SZ)}"
# Static app-shell cache names change on each release; data remains in IndexedDB.
sed "s/__BUILD__/$WB_BUILD_ID/g" "$WB_ROOT/static/sw.js" > "$WB_OUTPUT/static/sw.js"
printf '%s\n' "$WB_BUILD_ID" > "$WB_OUTPUT/BUILD_ID"
WB_ARCHIVE="$WB_ROOT/dist/whole-board-$WB_PLATFORM.tar.gz"
tar -C "$WB_ROOT/dist" -czf "$WB_ARCHIVE" "whole-board-$WB_PLATFORM"
if command -v sha256sum >/dev/null 2>&1; then (cd "$(dirname "$WB_ARCHIVE")" && sha256sum "$(basename "$WB_ARCHIVE")") > "$WB_ARCHIVE.sha256"; else (cd "$(dirname "$WB_ARCHIVE")" && shasum -a 256 "$(basename "$WB_ARCHIVE")") > "$WB_ARCHIVE.sha256"; fi
printf 'Release: %s\n' "$WB_ARCHIVE"
