#!/usr/bin/env bash
# Serve the desktop application with Dioxus live-reload.
#
# Usage: scripts/dev/serve-desktop.sh [--no-hot-patch] [--open] [extra dx serve args...]
#
# Rust hot-patching (Subsecond) is enabled by default; pass --no-hot-patch to
# disable it. RSX markup and string assets reload instantly either way.
# See https://dioxuslabs.com/learn/0.7/essentials/ui/hotreload/
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repository_root"

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: '$1' is required but was not found on PATH." >&2
    echo "Run this script from inside the development shell: nix develop" >&2
    exit 1
  fi
}

require_tool dx
require_tool cargo

hot_patch_args=(--hot-patch)
open_args=()
passthrough_args=()

while [ "$#" -gt 0 ]; do
  case "$1" in
    --hot-patch)
      hot_patch_args=(--hot-patch)
      shift
      ;;
    --no-hot-patch)
      hot_patch_args=()
      shift
      ;;
    --open)
      open_args=(--open)
      shift
      ;;
    --)
      shift
      passthrough_args=("$@")
      break
      ;;
    *)
      passthrough_args+=("$1")
      shift
      ;;
  esac
done

if [ "${POLYGLID_DATA_DIR:-}" = "" ]; then
  echo "info: POLYGLID_DATA_DIR is unset; the app will use the default data folder." >&2
  echo "info: set it (e.g. target/dev-data) to isolate dev-run data from real installs." >&2
fi

echo "Starting dx serve for polyglid-desktop (live reload active; press 'r' to force a full rebuild)..." >&2

exec dx serve \
  --desktop \
  --package polyglid-desktop \
  "${hot_patch_args[@]}" \
  "${open_args[@]}" \
  "${passthrough_args[@]}"
