#!/usr/bin/env bash
# Watch the desktop sources and restart the native GPUI application on changes.
#
# Usage: scripts/dev/dev-desktop.sh [extra cargo-watch args...]
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repository_root"

if ! command -v cargo-watch >/dev/null 2>&1 && ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required but was not found on PATH." >&2
  echo "Run this script from inside the development shell: nix develop" >&2
  exit 1
fi

if ! cargo watch --version >/dev/null 2>&1; then
  echo "error: cargo-watch is required for 'make dev'." >&2
  echo "Enter the pinned shell with 'nix develop' (it includes cargo-watch)." >&2
  exit 1
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/polyglid-workbench-target}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
# Native GPUI/Wasmtime dependencies are large; omitting debug info keeps the
# watcher below the size of common  tmpfs mounts used for /tmp.
export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-0}"

echo "Watching PolyGlid sources; the desktop app will restart after changes." >&2
exec cargo watch \
  --watch apps/desktop \
  --watch crates \
  --watch contracts \
  --exec "run --package polyglid-desktop" \
  "$@"
